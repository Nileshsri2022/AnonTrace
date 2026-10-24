use simple_signal_core::api::{SimpleSignalApp, TransportMode};
use simple_signal_core::net::relay::{blind_queue_id, RelayStore, serve_relay};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::net::TcpListener;

#[test]
fn blind_queue_id_is_deterministic_and_secret_specific() {
    let secret_a = [0x42u8; 32];
    let secret_b = [0x43u8; 32];
    assert_eq!(blind_queue_id(&secret_a), blind_queue_id(&secret_a));
    assert_ne!(blind_queue_id(&secret_a), blind_queue_id(&secret_b));
    assert_eq!(blind_queue_id(&secret_a).len(), 64);
}

#[test]
fn relay_store_send_poll_ack_and_auth() {
    let mut store = RelayStore::new();
    let blind = "abcd1234";
    // Unknown queue: poll is empty, send fails.
    assert!(store.poll(blind, None).is_empty());
    assert!(store.send(blind, b"key", "deadbeef").is_err());
    // Register, then wrong auth is rejected.
    store.register(blind, b"key").unwrap();
    assert!(store.send(blind, b"wrong", "deadbeef").is_err());
    // Correct auth: send two, poll all, poll since first, ACK first.
    let id1 = store.send(blind, b"key", "aa").unwrap();
    let id2 = store.send(blind, b"key", "bb").unwrap();
    assert_eq!(store.poll(blind, None).len(), 2);
    let after_first = store.poll(blind, Some(&id1));
    assert_eq!(after_first.len(), 1);
    assert_eq!(after_first[0].id, id2);
    assert!(store.ack(blind, &id1));
    assert!(!store.ack(blind, &id1));
    assert_eq!(store.poll(blind, None).len(), 1);
}

#[test]
fn relay_store_ttl_purges_old_messages() {
    let mut store = RelayStore::new();
    store.register("q1", b"k").unwrap();
    store.send("q1", b"k", "aa").unwrap();
    // Purge with now = far future: message (timestamp ~now) is older than TTL.
    store.purge_expired(10_000_000_000, 7 * 24 * 3600);
    assert!(store.poll("q1", None).is_empty());
}

async fn raw_http(port: u16, request: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut out = String::new();
    let _ = stream.read_to_string(&mut out).await;
    out
}

fn http_body(response: &str) -> &str {
    response.split("\r\n\r\n").nth(1).unwrap_or("")
}

#[tokio::test]
async fn relay_http_roundtrip() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let store = Arc::new(Mutex::new(RelayStore::new()));
    tokio::spawn(serve_relay(Arc::clone(&store), listener));

    let health = raw_http(port, "GET /health HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").await;
    assert!(health.contains("200 OK"));
    assert!(http_body(&health).contains("ok"));

    let reg = raw_http(port, "POST /q/blind123/register HTTP/1.1\r\nHost: x\r\nX-Sender-Auth: 6b6579\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
    assert!(reg.contains("200 OK"), "register: {}", reg);

    let send = raw_http(port, "POST /q/blind123 HTTP/1.1\r\nHost: x\r\nX-Sender-Auth: 6b6579\r\nContent-Length: 4\r\nConnection: close\r\n\r\nabcd").await;
    assert!(send.contains("200 OK"), "send: {}", send);
    assert!(http_body(&send).contains("\"id\":\"m1\""));

    let wrong = raw_http(port, "POST /q/blind123 HTTP/1.1\r\nHost: x\r\nX-Sender-Auth: 77726f6e67\r\nContent-Length: 4\r\nConnection: close\r\n\r\nabcd").await;
    assert!(wrong.contains("403"), "wrong auth: {}", wrong);

    let poll = raw_http(port, "GET /q/blind123/json?since=all HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").await;
    assert!(poll.contains("200 OK"));
    assert!(http_body(&poll).contains("\"message\":\"abcd\""));

    let ack = raw_http(port, "POST /q/blind123/ack/m1 HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
    assert!(http_body(&ack).contains("\"deleted\":true"));

    let poll2 = raw_http(port, "GET /q/blind123/json?since=all HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").await;
    assert_eq!(http_body(&poll2), "");
}

#[test]
fn blind_queue_and_sender_auth_persist_across_restart() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("test.db");
    let app = SimpleSignalApp::init(db.to_str().unwrap(), "pin123").unwrap();
    assert_eq!(app.transport_mode(), TransportMode::Normal);
    let blind1 = app.my_blind_queue_id();
    let auth1 = app.my_sender_auth_key_hex();
    assert_eq!(blind1.len(), 64);
    assert_ne!(auth1, hex::encode([0x11u8; 32]));
    app.set_transport_mode(TransportMode::Stealth);
    assert_eq!(app.transport_mode(), TransportMode::Stealth);
    drop(app);
    let app2 = SimpleSignalApp::init(db.to_str().unwrap(), "pin123").unwrap();
    assert_eq!(app2.my_blind_queue_id(), blind1);
    assert_eq!(app2.my_sender_auth_key_hex(), auth1);
    // Transport mode is a runtime toggle, not persisted (default Normal on restart).
    assert_eq!(app2.transport_mode(), TransportMode::Normal);
    // Contact link now carries the blind queue and the real sender auth.
    let link = app2.get_my_contact_link().unwrap();
    assert!(link.contains(&format!("queue={}", blind1)));
    assert!(link.contains(&format!("auth={}", auth1)));
    assert!(link.contains("relay=https://relay.anontrace.app"));
}
