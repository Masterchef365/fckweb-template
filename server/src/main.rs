use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use chat_common::*;
use framework::futures::lock::Mutex;
use framework::futures::{Sink, SinkExt};
use framework::io::FrameworkError;
use framework::tarpc::context::Context as TarpcContext;
use framework::{
    futures::StreamExt,
    tarpc::server::{BaseChannel, Channel},
    ServerFramework,
};
use tokio::sync::mpsc::Sender as TokioSender;
use tokio::sync::Mutex as TokioMutex;

pub const DEFINITELY_NOT_THE_PRIVATE_KEY: &[u8] = include_bytes!("localhost.key");

#[cfg(feature = "http")]
mod http;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    #[cfg(feature = "http")]
    tokio::spawn(http::http_server());

    chat_server().await
}

async fn chat_server() -> Result<()> {
    log::info!("Chat server");

    let endpoint = quic_session::server_endpoint(
        "0.0.0.0:9090".parse().unwrap(),
        chat_common::CERTIFICATE.to_vec(),
        DEFINITELY_NOT_THE_PRIVATE_KEY.to_vec(),
    )
    .await?;

    let mut shared = SharedData::default();
    shared
        .create_room(RoomDescription {
            name: "Default room".into(),
            long_desc: "This is the first room!".into(),
        })
        .await;

    let shared = Arc::new(TokioMutex::new(shared));

    while let Some(inc) = endpoint.accept().await {
        log::info!("New connection");
        let shared = shared.clone();

        tokio::spawn(async move {
            let sess = quic_session::server_connect(inc).await?;

            // Spawn the root service
            let (framework, channel) = ServerFramework::new(sess).await?;
            let transport = BaseChannel::with_defaults(channel);

            let server = ChatServer::new(framework, shared);

            let executor = transport.execute(ChatService::serve(server));

            tokio::spawn(executor.for_each(|response| async move {
                tokio::spawn(response);
            }));

            log::info!("Connection ended");

            Ok::<_, anyhow::Error>(())
        });
    }

    Ok(())
}

#[derive(Clone)]
struct ChatServer {
    framework: ServerFramework,
    shared: Arc<TokioMutex<SharedData>>,
}

#[derive(Default)]
struct SharedData {
    rooms: HashMap<String, Arc<TokioMutex<Room>>>,
}

type MessageSink =
    Arc<Mutex<dyn Sink<MessageMetaData, Error = FrameworkError> + Send + Sync + Unpin + 'static>>;

struct Room {
    desc: RoomDescription,
    connected: Vec<MessageSink>,
    tx: TokioSender<MessageMetaData>,
}

impl ChatServer {
    pub fn new(framework: ServerFramework, shared: Arc<TokioMutex<SharedData>>) -> Self {
        Self { framework, shared }
    }
}

impl ChatService for ChatServer {
    async fn create_room(self, _context: TarpcContext, desc: RoomDescription) -> bool {
        self.shared.lock().await.create_room(desc).await
    }

    async fn get_rooms(self, _context: TarpcContext) -> HashMap<String, RoomDescription> {
        let rooms = self.shared.lock().await.rooms.clone(); // note: relatively cheap
        let mut out_rooms = HashMap::new();
        for (name, room) in rooms {
            out_rooms.insert(name, room.lock().await.desc.clone());
        }
        out_rooms
    }

    async fn chat(
        self,
        _context: TarpcContext,
        room_name: String,
    ) -> Result<framework::BiStream<MessageMetaData, MessageMetaData>, ChatError> {
        let (handle, streamfut) = self.framework.accept_bistream();

        let shared = self.shared.clone();
        tokio::spawn(async move {
            let streams = streamfut.await?;
            let (sink, mut stream) = streams.split();

            let shared = shared.lock().await;
            let room_arc = shared.get_room(&room_name).await?;
            drop(shared);
            let mut room = room_arc.lock().await;
            room.connected.push(Arc::new(Mutex::new(sink)));

            let tx = room.tx.clone();
            drop(room);

            tokio::spawn(async move {
                while let Some(msg) = stream.next().await.transpose()? {
                    tx.send(msg).await?;
                }

                Ok::<_, anyhow::Error>(())
            });

            Ok::<_, anyhow::Error>(())
        });

        Ok(handle)
    }
}

impl SharedData {
    async fn get_room(&self, room_name: &str) -> Result<Arc<TokioMutex<Room>>, ChatError> {
        self.rooms
            .get(room_name)
            .ok_or_else(|| ChatError::RoomDoesNotExist(room_name.to_string()))
            .cloned()
    }

    async fn create_room(&mut self, desc: RoomDescription) -> bool {
        if self.rooms.contains_key(&desc.name) {
            false
        } else {
            self.rooms.insert(desc.name.clone(), Room::new(desc).await);
            true
        }
    }
}

impl Room {
    async fn new(desc: RoomDescription) -> Arc<TokioMutex<Self>> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);

        let inst = Self {
            tx,
            desc,
            connected: vec![],
        };

        let inst = Arc::new(TokioMutex::new(inst));

        let room = inst.clone();

        tokio::spawn(async move {
            // TODO: This is straightforward but slow!
            while let Some(msg) = rx.recv().await {
                let lck = room.lock().await;

                let mut handles = vec![];
                for conn in &lck.connected {
                    let conn = conn.clone();
                    let ptr = Arc::as_ptr(&conn) as *const () as usize;
                    let msg = msg.clone();
                    handles.push(tokio::spawn(async move {
                        let result = conn.lock().await.send(msg).await;
                        (ptr, result)
                    }));
                }

                drop(lck);

                // This may take awhile, so we've dropped the lock
                let mut del_indices = std::collections::HashSet::new();
                for handle in handles {
                    let (ptr, result) = handle.await.unwrap();
                    if let Err(e) = result {
                        log::error!("{}", e);
                        del_indices.insert(ptr);
                    }
                }

                let mut lck = room.lock().await;
                lck.connected.retain(|conn| {
                    let ptr = Arc::as_ptr(&conn) as *const () as usize;
                    !del_indices.contains(&ptr)
                });
            }

            Ok::<_, anyhow::Error>(())
        });

        inst
    }
}
