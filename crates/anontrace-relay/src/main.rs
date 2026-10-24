use std::sync::{Arc, Mutex};
use simple_signal_core::net::relay::{serve_relay, RelayStore};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(8080);
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    eprintln!("anontrace-relay listening on 127.0.0.1:{} (put TLS on port 443 in front of it)", port);
    serve_relay(Arc::new(Mutex::new(RelayStore::new())), listener).await
}
