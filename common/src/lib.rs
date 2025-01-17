use std::collections::HashMap;
use thiserror::Error;

use framework::BiStream;
use serde::{Deserialize, Serialize};

/// TLS certificate (self-signed for debug purposes)
pub const CERTIFICATE: &[u8] = include_bytes!("localhost.crt");
pub const CERTIFICATE_HASHES: &[u8] = include_bytes!("localhost.hex");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomDescription {
    pub name: String,
    pub long_desc: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageMetaData {
    pub username: String,
    pub user_color: [u8; 3],
    pub msg: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Error)]
pub enum ChatError {
    #[error("The requested room does not exist: {0}")]
    RoomDoesNotExist(String),
}

#[tarpc::service]
pub trait ChatService {
    /// Gets the rooms by name
    async fn get_rooms() -> HashMap<String, RoomDescription>;

    /// Returns true on success
    async fn create_room(desc: RoomDescription) -> bool;

    /// Connects to the given room
    async fn chat(
        room_name: String,
    ) -> Result<BiStream<MessageMetaData, MessageMetaData>, ChatError>;
}
