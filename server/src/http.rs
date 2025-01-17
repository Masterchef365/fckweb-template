//! From https://github.com/tokio-rs/axum/blob/main/examples/static-file-server/src/main.rs

use axum::Router;
use std::net::SocketAddr;
use tower_http::services::ServeDir;

pub async fn http_server() {
    let server = ServeDir::new("../client/dist/");
    let app = Router::new().nest_service("/", server);
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
