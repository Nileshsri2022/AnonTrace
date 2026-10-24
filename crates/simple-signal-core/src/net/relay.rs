//! Private relay for Option B (Normal Mode).
//!
//! The relay is intentionally dumb and zero-retention: it stores only a
//! blind queue id, a sender-auth hash, ciphertext (as hex, exactly as the
//! client sent it), a message id, and an arrival timestamp. It never sees
//! plaintext, the raw queue id, the queue secret, or any account.
//!
//! Wire shape (v1, ntfy-compatible read path):
//! - `POST /q/<blind_id>/register` with `X-Sender-Auth: <hex>` — create a queue.
//! - `POST /q/<blind_id>` with `X-Sender-Auth: <hex>`, body = payload hex — send.
//! - `GET /q/<blind_id>/json?since=<id|all>[&poll=1]` — NDJSON lines:
//!   `{"event":"message","id":"...","message":"<hex>","time":<unix>}`.
//!   Unknown queues return 200 with an empty body, not 404, so existence
//!   is not leaked. (SSE clients reconnect after the body closes; the
//!   Flutter client already retries with `sinceId`.)
//! - `POST /q/<blind_id>/ack/<msg_id>` — delete that message (zero retention).
//! - `GET /health` — `{"status":"ok"}`.
//!
//! This binary does NOT terminate TLS. Run it behind a reverse proxy
//! (nginx/Caddy) that provides port 443; the relay itself listens on
//! loopback HTTP. See `crates/anontrace-relay/README.md`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

type HmacSha256 = Hmac<Sha256>;

/// Server-side message TTL backstop (7 days). Delete-on-ACK is the
/// primary retention rule; TTL only cleans up un-ACKed messages.
pub const RELAY_TTL_SECONDS: u64 = 7 * 24 * 3600;

/// Derives the blind queue id from a per-inbox queue secret.
/// The relay never sees the secret or the raw queue id.
pub fn blind_queue_id(queue_secret: &[u8; 32]) -> String {
    let mut mac = HmacSha256::new_from_slice(queue_secret)
        .expect("HMAC can take key of any size");
    mac.update(b"anontrace-inbound-v1");
    hex::encode(mac.finalize().into_bytes())
}

/// Hash of a sender auth key, as stored by the relay. Never store the key.
pub fn sender_auth_hash(key: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(key);
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum RelayError {
    #[error("Unknown queue")]
    UnknownQueue,
    #[error("Sender authentication failed")]
    AuthFailed,
    #[error("Queue already registered with a different sender auth key")]
    AlreadyRegistered,
}

#[derive(Clone, Debug)]
pub struct RelayMessage {
    pub id: String,
    pub payload_hex: String,
    pub timestamp: u64,
}

struct RelayQueue {
    sender_auth_hash: [u8; 32],
    messages: Vec<RelayMessage>,
    next_id: u64,
}

#[derive(Default)]
pub struct RelayStore {
    queues: HashMap<String, RelayQueue>,
}

impl RelayStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        blind_id: &str,
        sender_auth_key: &[u8],
    ) -> Result<(), RelayError> {
        let hash = sender_auth_hash(sender_auth_key);
        match self.queues.get(blind_id) {
            Some(q) if q.sender_auth_hash != hash => Err(RelayError::AlreadyRegistered),
            Some(_) => Ok(()),
            None => {
                self.queues.insert(
                    blind_id.to_string(),
                    RelayQueue {
                        sender_auth_hash: hash,
                        messages: Vec::new(),
                        next_id: 1,
                    },
                );
                Ok(())
            }
        }
    }

    pub fn send(
        &mut self,
        blind_id: &str,
        sender_auth_key: &[u8],
        payload_hex: &str,
    ) -> Result<String, RelayError> {
        let hash = sender_auth_hash(sender_auth_key);
        let queue = self
            .queues
            .get_mut(blind_id)
            .ok_or(RelayError::UnknownQueue)?;
        if queue.sender_auth_hash != hash {
            return Err(RelayError::AuthFailed);
        }
        let id = format!("m{}", queue.next_id);
        queue.next_id += 1;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        queue.messages.push(RelayMessage {
            id: id.clone(),
            payload_hex: payload_hex.to_string(),
            timestamp,
        });
        Ok(id)
    }

    /// Messages after `since_id` (`None` or `"all"` = all messages).
    /// Unknown queues return an empty vec (no existence leak).
    pub fn poll(&self, blind_id: &str, since_id: Option<&str>) -> Vec<RelayMessage> {
        let Some(queue) = self.queues.get(blind_id) else {
            return Vec::new();
        };
        match since_id {
            None | Some("all") | Some("") => queue.messages.clone(),
            Some(since) => {
                let mut found = false;
                let mut out = Vec::new();
                for msg in &queue.messages {
                    if found {
                        out.push(msg.clone());
                    } else if msg.id == since {
                        found = true;
                    }
                }
                out
            }
        }
    }

    pub fn ack(&mut self, blind_id: &str, msg_id: &str) -> bool {
        let Some(queue) = self.queues.get_mut(blind_id) else {
            return false;
        };
        let before = queue.messages.len();
        queue.messages.retain(|m| m.id != msg_id);
        queue.messages.len() != before
    }

    pub fn purge_expired(&mut self, now: u64, ttl_seconds: u64) {
        for queue in self.queues.values_mut() {
            queue
                .messages
                .retain(|m| now.saturating_sub(m.timestamp) <= ttl_seconds);
        }
    }
}

/// Serves the relay HTTP API on `listener` forever (until the listener
/// errors). Each connection is handled on its own task.
pub async fn serve_relay(
    store: Arc<Mutex<RelayStore>>,
    listener: TcpListener,
) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let store = Arc::clone(&store);
        tokio::spawn(async move {
            let _ = handle_connection(store, stream).await;
        });
    }
}

async fn handle_connection(
    store: Arc<Mutex<RelayStore>>,
    mut stream: TcpStream,
) -> std::io::Result<()> {
    // Read headers (cap 64 KiB) then body by Content-Length (cap 128 KiB).
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > 64 * 1024 {
            return Ok(());
        }
    };
    let header_text = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = header_text.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_uppercase();
    let raw_path = parts.next().unwrap_or("").to_string();
    let mut content_length: usize = 0;
    let mut sender_auth_hex = String::new();
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0).min(128 * 1024);
        } else if let Some(v) = lower.strip_prefix("x-sender-auth:") {
            sender_auth_hex = v.trim().to_string();
        }
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);
    let body_text = String::from_utf8_lossy(&body).trim().to_string();

    let (path, query) = match raw_path.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (raw_path.clone(), String::new()),
    };
    let query_params: HashMap<String, String> = query
        .split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let mut kv = pair.splitn(2, '=');
            Some((kv.next()?.to_string(), kv.next().unwrap_or("").to_string()))
        })
        .collect();

    let (status, content_type, response_body) = route(&store, &method, &path, &query_params, &sender_auth_hex, &body_text);
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        content_type,
        response_body.len(),
        response_body
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

fn route(
    store: &Arc<Mutex<RelayStore>>,
    method: &str,
    path: &str,
    query: &HashMap<String, String>,
    sender_auth_hex: &str,
    body: &str,
) -> (&'static str, &'static str, String) {
    if method == "GET" && path == "/health" {
        return ("200 OK", "application/json", r#"{"status":"ok"}"#.to_string());
    }
    let Some(rest) = path.strip_prefix("/q/") else {
        return ("404 Not Found", "application/json", r#"{"error":"not found"}"#.to_string());
    };
    let segments: Vec<&str> = rest.split('/').collect();
    let blind_id = segments[0];
    if blind_id.is_empty() {
        return ("404 Not Found", "application/json", r#"{"error":"not found"}"#.to_string());
    }
    let sender_auth = hex::decode(sender_auth_hex).unwrap_or_default();

    // POST /q/<id>/register
    if method == "POST" && segments.len() == 2 && segments[1] == "register" {
        if sender_auth.is_empty() {
            return ("400 Bad Request", "application/json", r#"{"error":"sender auth required"}"#.to_string());
        }
        let mut store = store.lock().unwrap();
        return match store.register(blind_id, &sender_auth) {
            Ok(()) => ("200 OK", "application/json", r#"{"registered":true}"#.to_string()),
            Err(_) => ("403 Forbidden", "application/json", r#"{"error":"already registered"}"#.to_string()),
        };
    }
    // POST /q/<id>/ack/<msg_id>
    if method == "POST" && segments.len() == 3 && segments[1] == "ack" {
        let mut store = store.lock().unwrap();
        let deleted = store.ack(blind_id, segments[2]);
        return (
            "200 OK",
            "application/json",
            format!(r#"{{"deleted":{}}}"#, deleted),
        );
    }
    // POST /q/<id> — send
    if method == "POST" && segments.len() == 1 {
        let mut store = store.lock().unwrap();
        return match store.send(blind_id, &sender_auth, body) {
            Ok(id) => ("200 OK", "application/json", format!(r#"{{"id":"{}"}}"#, id)),
            Err(RelayError::UnknownQueue) => ("404 Not Found", "application/json", r#"{"error":"unknown queue"}"#.to_string()),
            Err(_) => ("403 Forbidden", "application/json", r#"{"error":"sender auth failed"}"#.to_string()),
        };
    }
    // GET /q/<id>/json — poll / SSE (NDJSON, then close; client reconnects)
    if method == "GET" && segments.len() == 2 && segments[1] == "json" {
        let since = query.get("since").map(|s| s.as_str());
        let store = store.lock().unwrap();
        let messages = store.poll(blind_id, since);
        let mut out = String::new();
        for msg in messages {
            out.push_str(&format!(
                "{{\"event\":\"message\",\"id\":\"{}\",\"message\":\"{}\",\"time\":{}}}\n",
                msg.id, msg.payload_hex, msg.timestamp
            ));
        }
        return ("200 OK", "application/x-ndjson", out);
    }
    ("404 Not Found", "application/json", r#"{"error":"not found"}"#.to_string())
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}
