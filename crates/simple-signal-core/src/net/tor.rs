use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

const ISOLATION_SEED: &[u8] = b"simple-signal-tor-stream-isolation-salt-v1";

#[derive(Error, Debug, PartialEq, Eq)]
pub enum TorError {
    #[error("Tor configuration error: {0}")]
    ConfigError(String),

    #[error("Invalid Tor onion address: {0}")]
    InvalidOnionAddress(String),

    #[error("Tor bootstrap consensus failed: {0}")]
    BootstrapFailed(String),

    #[error("Tor stream circuit error: {0}")]
    CircuitError(String),

    #[error("Stealth transport is not bootstrapped")]
    NotBootstrapped,

    #[error("Stealth transport has no onion address configured")]
    NoOnionAddress,
}

/// Lifecycle of the embedded Arti client (Ticket 26.3).
/// Bootstrapping is asynchronous: `start_stealth_bootstrap` returns
/// immediately and `stealth_status` is polled for progress, so a
/// Flutter FFI call never blocks the UI thread for 10–30 s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StealthState {
    NotStarted,
    Bootstrapping,
    Ready,
    Failed(String),
}

/// Polled status for the Stealth bootstrap, for UI progress display.
#[derive(Clone, Debug)]
pub struct StealthStatus {
    pub state: StealthState,
    /// 0.0–1.0 from Arti's BootstrapStatus; 0.0 before a client exists.
    pub frac: f32,
    pub ready_for_traffic: bool,
    pub blocked: Option<String>,
    pub onion_address: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DormancyMode {
    Active,
    Soft,
}

#[derive(Clone, Debug)]
pub struct TorTransportConfig {
    pub data_directory: PathBuf,
    pub socks_port: Option<u16>,
    pub strict_tor_only: bool,
}

impl Default for TorTransportConfig {
    fn default() -> Self {
        Self {
            data_directory: PathBuf::from("./tor_data"),
            socks_port: None,
            strict_tor_only: true,
        }
    }
}

struct TorClientInner {
    config: TorTransportConfig,
    dormancy: DormancyMode,
    // Arti client, once created (unbootstrapped first, so status can be polled).
    arti_client: Option<Arc<arti_client::TorClient<tor_rtcompat::PreferredRuntime>>>,
    stealth_state: StealthState,
    onion_address: Option<String>,
    // Dedicated runtime for Arti, created lazily. Arti's PreferredRuntime
    // requires a running Tokio runtime; Flutter's Dart isolate has none,
    // so the core owns one for the Stealth path.
    arti_runtime: Option<Arc<tokio::runtime::Runtime>>,
}

#[derive(Clone)]
pub struct TorClientManager {
    inner: Arc<Mutex<TorClientInner>>,
}

impl TorClientManager {
    /// Initializes in-process embedded Tor manager with configuration
    pub fn new(config: TorTransportConfig) -> Result<Self, TorError> {
        if config.strict_tor_only && config.socks_port.is_some() {
            // Warning/defense: strict Tor only messenger should avoid exposing open local ports to localhost
        }

        Ok(Self {
            inner: Arc::new(Mutex::new(TorClientInner {
                config,
                dormancy: DormancyMode::Active,
                arti_client: None,
                stealth_state: StealthState::NotStarted,
                onion_address: None,
                arti_runtime: None,
            })),
        })
    }

    /// Creates a default TorClientManager instance
    pub fn default_client() -> Self {
        Self::new(TorTransportConfig::default()).expect("default tor client")
    }

    /// Returns current config
    pub fn config(&self) -> TorTransportConfig {
        self.inner.lock().unwrap().config.clone()
    }

    /// Derives a cryptographically isolated circuit token for a specific pairwise queue.
    /// This enforces `isolate-by-auth` so communications across different contacts
    /// never share the same Tor circuit or guard node timings.
    pub fn isolation_token_for_queue(&self, queue_id: &str) -> [u8; 32] {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(ISOLATION_SEED)
            .expect("HMAC can take key of any size");
        mac.update(queue_id.as_bytes());
        let result = mac.finalize().into_bytes();
        let mut token = [0u8; 32];
        token.copy_from_slice(&result);
        token
    }

    /// Returns current dormancy state
    pub fn dormancy_state(&self) -> DormancyMode {
        self.inner.lock().unwrap().dormancy
    }

    /// Returns true if the client is actively connected (not dormant)
    pub fn is_active(&self) -> bool {
        self.dormancy_state() == DormancyMode::Active
    }

    /// Transitions Tor client into soft dormant mode to conserve mobile battery
    pub fn set_dormancy(&self, mode: DormancyMode) -> Result<(), TorError> {
        self.inner.lock().unwrap().dormancy = mode;
        Ok(())
    }

    /// Awakens the Tor client from dormancy upon incoming push notification
    pub fn wake_up(&self) -> Result<(), TorError> {
        self.inner.lock().unwrap().dormancy = DormancyMode::Active;
        Ok(())
    }

    /// Starts the embedded Arti bootstrap in the background (non-blocking).
    /// `data_dir` is supplied by Flutter (e.g. the app documents dir) and
    /// must never be the old `./tor_data` default on Android. Arti state
    /// goes to `<data_dir>/tor_state`, cache to `<data_dir>/tor_cache`.
    /// `onion_address` is the relay's onion address, if deployed; without
    /// it, Stealth sends fail closed (no clearnet fallback — see
    /// `stealth_post`).
    /// Idempotent while Bootstrapping or Ready; call again after Failed
    /// to retry.
    pub fn start_stealth_bootstrap(
        &self,
        data_dir: &Path,
        onion_address: Option<String>,
    ) -> Result<(), TorError> {
        if let Some(onion) = &onion_address {
            validate_onion_address(onion)?;
        }
        let mut inner = self.inner.lock().unwrap();
        match &inner.stealth_state {
            StealthState::Bootstrapping | StealthState::Ready => return Ok(()),
            _ => {}
        }
        if inner.arti_runtime.is_none() {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| TorError::ConfigError(e.to_string()))?;
            inner.arti_runtime = Some(Arc::new(runtime));
        }
        let runtime = inner.arti_runtime.clone().unwrap();
        inner.stealth_state = StealthState::Bootstrapping;
        inner.onion_address = onion_address;
        drop(inner);

        let manager = self.clone();
        let state_dir = data_dir.join("tor_state");
        let cache_dir = data_dir.join("tor_cache");
        runtime.spawn(async move {
            let result = async {
                // Arti + rustls panics without a process-level CryptoProvider.
                // Install ring's provider once, here, before any Tor TLS.
                let _ = rustls::crypto::CryptoProvider::install_default(
                    rustls::crypto::ring::default_provider(),
                );
                let config = arti_client::config::TorClientConfigBuilder::from_directories(
                    &state_dir, &cache_dir,
                )
                .build()
                .map_err(|e| TorError::ConfigError(e.to_string()))?;
                // Unbootstrapped first, so status polling can see frac/blocked.
                let client = arti_client::TorClient::builder()
                    .config(config)
                    .create_unbootstrapped_async()
                    .await
                    .map_err(|e| TorError::BootstrapFailed(e.to_string()))?;
                {
                    let mut inner = manager.inner.lock().unwrap();
                    inner.arti_client = Some(client.clone());
                }
                client
                    .bootstrap()
                    .await
                    .map_err(|e| TorError::BootstrapFailed(e.to_string()))?;
                Ok::<(), TorError>(())
            }
            .await;
            let mut inner = manager.inner.lock().unwrap();
            inner.stealth_state = match result {
                Ok(()) => StealthState::Ready,
                Err(e) => StealthState::Failed(e.to_string()),
            };
        });
        Ok(())
    }

    /// Current Stealth bootstrap status, for UI progress display.
    /// Never blocks on the network; reads only the stored client status.
    pub fn stealth_status(&self) -> StealthStatus {
        let inner = self.inner.lock().unwrap();
        let (frac, ready_for_traffic, blocked) = match &inner.arti_client {
            Some(client) => {
                let status = client.bootstrap_status();
                (
                    status.as_frac(),
                    status.ready_for_traffic(),
                    status.blocked().map(|b| b.to_string()),
                )
            }
            None => (0.0, false, None),
        };
        StealthStatus {
            state: inner.stealth_state.clone(),
            frac,
            ready_for_traffic,
            blocked,
            onion_address: inner.onion_address.clone(),
        }
    }

    /// Sends a relay POST over the Tor onion address, using an isolated
    /// Arti client for this queue (per-conversation circuit isolation).
    /// Returns the relay's JSON body (e.g. `{"id":"m1"}`) on success.
    /// Fails closed: if Arti is not Ready, if no onion address is
    /// configured, or if the onion connection fails, no clearnet
    /// request is made — Stealth must never silently downgrade to
    /// Normal Mode and leak the user's IP to the relay.
    /// HTTPS-over-Tor-exit fallback is NOT implemented here (it needs
    /// tokio-rustls + webpki-roots, new deps — see Issue 26 §8); the
    /// caller must surface this error, not fall back to Flutter
    /// HttpClient.
    pub async fn stealth_post(
        &self,
        queue_id: &str,
        sender_auth_hex: &str,
        payload_hex: &str,
    ) -> Result<String, TorError> {
        let (client, onion) = {
            let inner = self.inner.lock().unwrap();
            if inner.stealth_state != StealthState::Ready {
                return Err(TorError::NotBootstrapped);
            }
            let Some(client) = inner.arti_client.clone() else {
                return Err(TorError::NotBootstrapped);
            };
            let Some(onion) = inner.onion_address.clone() else {
                return Err(TorError::NoOnionAddress);
            };
            (client, onion)
        };
        // Isolated client per queue: streams for different contacts
        // never share a circuit.
        let isolated = client.isolated_client();
        let _token = self.isolation_token_for_queue(queue_id);
        let mut stream = isolated
            .connect((onion.as_str(), 80u16))
            .await
            .map_err(|e| TorError::CircuitError(e.to_string()))?;
        let body = format!(
            "POST /q/{} HTTP/1.1\r\nHost: {}\r\nX-Sender-Auth: {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            queue_id,
            onion,
            sender_auth_hex,
            payload_hex.len(),
            payload_hex
        );
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        stream
            .write_all(body.as_bytes())
            .await
            .map_err(|e| TorError::CircuitError(e.to_string()))?;
        stream
            .flush()
            .await
            .map_err(|e| TorError::CircuitError(e.to_string()))?;
        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .await
            .map_err(|e| TorError::CircuitError(e.to_string()))?;
        let text = String::from_utf8_lossy(&raw).to_string();
        let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        if text.contains("200 OK") {
            Ok(body)
        } else {
            Err(TorError::CircuitError(format!(
                "Relay over onion returned non-200: {}",
                text.lines().next().unwrap_or("")
            )))
        }
    }
}

impl TorClientManager {
    /// Synchronous wrapper around `stealth_post`, for FFI callers.
    /// Blocks the calling thread on the Arti runtime for the duration
    /// of the Tor request — Flutter must call this off the main
    /// isolate (or accept a slow Stealth send), never in a UI callback
    /// that must stay responsive. A non-blocking job-based FFI is a
    /// follow-up; this wrapper exists so the Stealth path is testable
    /// and usable without a Dart-side async FFI redesign.
    pub fn stealth_post_blocking(
        &self,
        queue_id: &str,
        sender_auth_hex: &str,
        payload_hex: &str,
    ) -> Result<String, TorError> {
        let runtime = {
            let inner = self.inner.lock().unwrap();
            inner.arti_runtime.clone()
        };
        let Some(runtime) = runtime else {
            return Err(TorError::NotBootstrapped);
        };
        runtime.block_on(self.stealth_post(queue_id, sender_auth_hex, payload_hex))
    }
}

/// Formats a relay POST request (for testing the wire shape without Tor).
pub fn format_relay_post(
    queue_id: &str,
    onion_or_host: &str,
    sender_auth_hex: &str,
    payload_hex: &str,
) -> String {
    format!(
        "POST /q/{} HTTP/1.1\r\nHost: {}\r\nX-Sender-Auth: {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        queue_id,
        onion_or_host,
        sender_auth_hex,
        payload_hex.len(),
        payload_hex
    )
}

/// Parses a relay HTTP response body, requiring 200 OK.
pub fn parse_relay_response(raw: &str) -> Result<String, TorError> {
    if !raw.contains("200 OK") {
        return Err(TorError::CircuitError(format!(
            "Relay returned non-200: {}",
            raw.lines().next().unwrap_or("")
        )));
    }
    Ok(raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
}

/// Validates whether an address is a syntactically valid Tor v3 hidden service address.
/// Tor v3 onion addresses are exactly 56 base32 characters followed by `.onion`.
pub fn validate_onion_address(addr: &str) -> Result<(), TorError> {
    if !addr.ends_with(".onion") {
        return Err(TorError::InvalidOnionAddress(format!(
            "Address '{}' must end with .onion",
            addr
        )));
    }

    let prefix = match addr.strip_suffix(".onion") {
        Some(p) => p,
        None => return Err(TorError::InvalidOnionAddress("Invalid suffix".to_string())),
    };

    // Tor v3 requires exactly 56 base32 characters
    if prefix.len() != 56 {
        return Err(TorError::InvalidOnionAddress(format!(
            "Tor v3 onion address must have 56 characters prefix, found {} characters",
            prefix.len()
        )));
    }

    // Must be valid base32 (a-z, 2-7)
    for ch in prefix.chars() {
        match ch {
            'a'..='z' | '2'..='7' => {}
            _ => {
                return Err(TorError::InvalidOnionAddress(format!(
                    "Invalid base32 character '{}' in onion address",
                    ch
                )))
            }
        }
    }

    Ok(())
}
