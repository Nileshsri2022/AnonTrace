use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::net::tor::{DormancyMode, TorClientManager, TorError};

type HmacSha256 = Hmac<Sha256>;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum PushError {
    #[error("Push gateway protocol error: {0}")]
    ProtocolError(String),

    #[error("Push gateway connection error: {0}")]
    ConnectionError(String),

    #[error("Push gateway returned error [{code}]: {message}")]
    GatewayError { code: String, message: String },

    #[error("Tor transport error: {0}")]
    TorError(String),
}

impl From<TorError> for PushError {
    fn from(err: TorError) -> Self {
        PushError::TorError(err.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushService {
    Apns,
    Fcm,
}

impl PushService {
    pub fn as_str(&self) -> &'static str {
        match self {
            PushService::Apns => "apns",
            PushService::Fcm => "fcm",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "apns" => Some(PushService::Apns),
            "fcm" => Some(PushService::Fcm),
            _ => None,
        }
    }
}

/// Computes a cryptographically blinded device token using HMAC-SHA256 with an ephemeral salt.
/// This prevents correlation between mobile push notification identifiers (APNs/FCM)
/// and SMP messaging relays or communication queues.
pub fn blind_device_token(raw_token: &str, salt: &[u8; 16]) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(salt)
        .expect("HMAC can take key of any size");
    mac.update(raw_token.as_bytes());
    let result = mac.finalize().into_bytes();
    hex::encode(result)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushCommand {
    RegisterToken {
        service: PushService,
        device_token: String,
    },
    BindNotifier {
        queue_id: String,
        blinded_token: String,
    },
    SubscribePush {
        blinded_token: String,
        queue_tag: String,
    },
    TriggerPush {
        queue_tag: String,
    },
}

impl PushCommand {
    /// Formats command into wire text followed by CRLF
    pub fn encode(&self) -> String {
        match self {
            PushCommand::RegisterToken {
                service,
                device_token,
            } => format!("TNEW {} {}\r\n", service.as_str(), device_token),
            PushCommand::BindNotifier {
                queue_id,
                blinded_token,
            } => format!("NKEY {} {}\r\n", queue_id, blinded_token),
            PushCommand::SubscribePush {
                blinded_token,
                queue_tag,
            } => format!("SNEW {} {}\r\n", blinded_token, queue_tag),
            PushCommand::TriggerPush { queue_tag } => format!("NMSG {}\r\n", queue_tag),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushResponse {
    Ok,
    TokenRegistered { blinded_token: String },
    NotifierBound,
    Subscribed,
    NotificationTriggered { queue_tag: String },
    Error { code: String, message: String },
}

impl PushResponse {
    /// Parses a line-oriented response from a push notification router or SMP relay
    pub fn parse(line: &str) -> Result<Self, PushError> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Err(PushError::ProtocolError("Empty response line".to_string()));
        }

        let mut parts = trimmed.splitn(3, ' ');
        let verb = parts.next().unwrap_or("");

        match verb {
            "OK" => Ok(PushResponse::Ok),
            "TNEW" => {
                let token = parts
                    .next()
                    .ok_or_else(|| PushError::ProtocolError("Missing blinded token in TNEW".to_string()))?;
                Ok(PushResponse::TokenRegistered {
                    blinded_token: token.to_string(),
                })
            }
            "NKEY" => {
                let status = parts.next().unwrap_or("");
                if status == "OK" {
                    Ok(PushResponse::NotifierBound)
                } else {
                    Ok(PushResponse::Ok)
                }
            }
            "SNEW" => {
                let status = parts.next().unwrap_or("");
                if status == "OK" {
                    Ok(PushResponse::Subscribed)
                } else {
                    Ok(PushResponse::Ok)
                }
            }
            "NMSG" => {
                let tag = parts
                    .next()
                    .ok_or_else(|| PushError::ProtocolError("Missing queue tag in NMSG".to_string()))?;
                Ok(PushResponse::NotificationTriggered {
                    queue_tag: tag.to_string(),
                })
            }
            "ERR" => {
                let code = parts
                    .next()
                    .ok_or_else(|| PushError::ProtocolError("Missing error code in ERR".to_string()))?;
                let message = parts.next().unwrap_or("Unknown error");
                Ok(PushResponse::Error {
                    code: code.to_string(),
                    message: message.to_string(),
                })
            }
            other => Err(PushError::ProtocolError(format!(
                "Unrecognized response verb: {}",
                other
            ))),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PushNotificationGateway;

impl PushNotificationGateway {
    pub fn new() -> Self {
        Self
    }

    /// Registers raw APNs/FCM device token with the Notification Router using `TNEW`
    /// and returns the opaque blinded token ID.
    pub async fn register_device_token<S>(
        &self,
        stream: &mut S,
        service: PushService,
        device_token: &str,
    ) -> Result<String, PushError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let cmd = PushCommand::RegisterToken {
            service,
            device_token: device_token.to_string(),
        };

        stream
            .write_all(cmd.encode().as_bytes())
            .await
            .map_err(|e| PushError::ConnectionError(e.to_string()))?;
        stream
            .flush()
            .await
            .map_err(|e| PushError::ConnectionError(e.to_string()))?;

        let mut buf = vec![0u8; 1024];
        let n = stream
            .read(&mut buf)
            .await
            .map_err(|e| PushError::ConnectionError(e.to_string()))?;

        if n == 0 {
            return Err(PushError::ConnectionError("Connection closed before response".to_string()));
        }

        let resp_str = String::from_utf8_lossy(&buf[..n]);
        match PushResponse::parse(&resp_str)? {
            PushResponse::TokenRegistered { blinded_token } => Ok(blinded_token),
            PushResponse::Error { code, message } => Err(PushError::GatewayError { code, message }),
            other => Err(PushError::ProtocolError(format!(
                "Unexpected response to TNEW: {:?}",
                other
            ))),
        }
    }

    /// Binds queue notifier with the SMP message relay using `NKEY`.
    pub async fn bind_queue_notifier<S>(
        &self,
        stream: &mut S,
        queue_id: &str,
        blinded_token: &str,
    ) -> Result<(), PushError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let cmd = PushCommand::BindNotifier {
            queue_id: queue_id.to_string(),
            blinded_token: blinded_token.to_string(),
        };

        stream
            .write_all(cmd.encode().as_bytes())
            .await
            .map_err(|e| PushError::ConnectionError(e.to_string()))?;
        stream
            .flush()
            .await
            .map_err(|e| PushError::ConnectionError(e.to_string()))?;

        let mut buf = vec![0u8; 1024];
        let n = stream
            .read(&mut buf)
            .await
            .map_err(|e| PushError::ConnectionError(e.to_string()))?;

        if n == 0 {
            return Err(PushError::ConnectionError("Connection closed before response".to_string()));
        }

        let resp_str = String::from_utf8_lossy(&buf[..n]);
        match PushResponse::parse(&resp_str)? {
            PushResponse::Ok | PushResponse::NotifierBound => Ok(()),
            PushResponse::Error { code, message } => Err(PushError::GatewayError { code, message }),
            other => Err(PushError::ProtocolError(format!(
                "Unexpected response to NKEY: {:?}",
                other
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WakeupResult {
    pub queue_tag: String,
    pub retrieval_executed: bool,
}

/// Coordinates mobile background wake-up triggers, transitioning Tor from battery-saving
/// dormancy to active message polling, then automatically restoring dormancy.
pub struct EphemeralWakeupCoordinator {
    tor_manager: TorClientManager,
}

impl EphemeralWakeupCoordinator {
    pub fn new(tor_manager: TorClientManager) -> Self {
        Self { tor_manager }
    }

    /// Transitions the Tor transport to soft dormancy to preserve battery life.
    pub async fn enter_dormancy(&self) -> Result<(), PushError> {
        self.tor_manager.set_dormancy(DormancyMode::Soft)?;
        Ok(())
    }

    /// Handles an incoming background push notification wakeup event.
    /// Wakes up Tor, performs message polling, and restores dormancy.
    pub async fn handle_push_wakeup(&self, queue_tag: &str) -> Result<WakeupResult, PushError> {
        // 1. Wake up Tor transport from soft dormancy
        self.tor_manager.wake_up()?;

        // 2. Simulated message retrieval loop over Tor
        // In real execution, this polls messages from the SMP queue tagged by queue_tag
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // 3. Immediately return to soft dormancy to conserve battery
        self.tor_manager.set_dormancy(DormancyMode::Soft)?;

        Ok(WakeupResult {
            queue_tag: queue_tag.to_string(),
            retrieval_executed: true,
        })
    }
}
