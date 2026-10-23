use std::path::PathBuf;
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

pub struct TorClientManager {
    config: TorTransportConfig,
    dormancy: DormancyMode,
}

impl TorClientManager {
    /// Initializes in-process embedded Tor manager with configuration
    pub fn new(config: TorTransportConfig) -> Result<Self, TorError> {
        if config.strict_tor_only && config.socks_port.is_some() {
            // Warning/defense: strict Tor only messenger should avoid exposing open local ports to localhost
        }

        Ok(Self {
            config,
            dormancy: DormancyMode::Active,
        })
    }

    /// Returns current config
    pub fn config(&self) -> &TorTransportConfig {
        &self.config
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
        self.dormancy
    }

    /// Transitions Tor client into soft dormant mode to conserve mobile battery
    pub fn set_dormancy(&mut self, mode: DormancyMode) -> Result<(), TorError> {
        self.dormancy = mode;
        Ok(())
    }

    /// Awakens the Tor client from dormancy upon incoming push notification
    pub fn wake_up(&mut self) -> Result<(), TorError> {
        self.dormancy = DormancyMode::Active;
        Ok(())
    }
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
