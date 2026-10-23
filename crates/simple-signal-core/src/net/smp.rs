use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SmpError {
    #[error("SMP protocol error: {0}")]
    ProtocolError(String),

    #[error("SMP relay connection closed unexpectedly")]
    ConnectionClosed,

    #[error("SMP relay returned error [{code}]: {message}")]
    RelayError { code: String, message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SmpCommand {
    New {
        capacity: Option<u32>,
    },
    SetSenderKey {
        queue_id: String,
        sender_auth_key: String,
    },
    Send {
        queue_id: String,
        corr_id: String,
        payload: Vec<u8>,
    },
    Subscribe {
        queue_id: String,
    },
    Ack {
        queue_id: String,
        message_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SmpResponse {
    Ok,
    QueueCreated {
        queue_id: String,
        auth_token: String,
    },
    MessageSent {
        message_id: String,
    },
    MessageReceived {
        message_id: String,
        payload: Vec<u8>,
    },
    Error {
        code: String,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueStatus {
    Uninitialized,
    Allocated {
        queue_id: String,
        auth_token: String,
    },
    Secured {
        queue_id: String,
        sender_auth_key: String,
    },
    Closed,
}

pub struct SmpQueueSession {
    status: QueueStatus,
}

impl Default for SmpQueueSession {
    fn default() -> Self {
        Self::new()
    }
}

impl SmpQueueSession {
    pub fn new() -> Self {
        Self {
            status: QueueStatus::Uninitialized,
        }
    }

    pub fn status(&self) -> QueueStatus {
        self.status.clone()
    }

    pub fn queue_id(&self) -> Option<String> {
        match &self.status {
            QueueStatus::Allocated { queue_id, .. } | QueueStatus::Secured { queue_id, .. } => {
                Some(queue_id.clone())
            }
            _ => None,
        }
    }

    pub fn on_queue_allocated(&mut self, queue_id: &str, auth_token: &str) -> Result<(), SmpError> {
        self.status = QueueStatus::Allocated {
            queue_id: queue_id.to_string(),
            auth_token: auth_token.to_string(),
        };
        Ok(())
    }

    pub fn configure_sender_key(&mut self, sender_auth_key: &str) -> Result<(), SmpError> {
        match &self.status {
            QueueStatus::Allocated { queue_id, .. } => {
                self.status = QueueStatus::Secured {
                    queue_id: queue_id.clone(),
                    sender_auth_key: sender_auth_key.to_string(),
                };
                Ok(())
            }
            _ => Err(SmpError::ProtocolError(
                "Queue not allocated. Must call NEW before SKEY.".to_string(),
            )),
        }
    }
}

/// Encodes an SMP command into wire bytes.
pub fn encode_command(cmd: &SmpCommand) -> Vec<u8> {
    let mut bytes = Vec::new();
    match cmd {
        SmpCommand::New { capacity } => {
            if let Some(cap) = capacity {
                bytes.extend_from_slice(format!("NEW {}\r\n", cap).as_bytes());
            } else {
                bytes.extend_from_slice(b"NEW\r\n");
            }
        }
        SmpCommand::SetSenderKey {
            queue_id,
            sender_auth_key,
        } => {
            bytes.extend_from_slice(
                format!("SKEY {} {}\r\n", queue_id, sender_auth_key).as_bytes(),
            );
        }
        SmpCommand::Send {
            queue_id,
            corr_id,
            payload,
        } => {
            let header = format!("SEND {} {} {}\r\n", queue_id, corr_id, payload.len());
            bytes.extend_from_slice(header.as_bytes());
            bytes.extend_from_slice(payload);
            bytes.extend_from_slice(b"\r\n");
        }
        SmpCommand::Subscribe { queue_id } => {
            bytes.extend_from_slice(format!("SUB {}\r\n", queue_id).as_bytes());
        }
        SmpCommand::Ack {
            queue_id,
            message_id,
        } => {
            bytes.extend_from_slice(format!("ACK {} {}\r\n", queue_id, message_id).as_bytes());
        }
    }
    bytes
}

/// Tries to decode one complete SMP response from a buffer, consuming the parsed bytes.
pub fn try_decode_response_from_buf(buf: &mut Vec<u8>) -> Result<Option<SmpResponse>, SmpError> {
    // Look for first \r\n
    let header_end = match buf.windows(2).position(|w| w == b"\r\n") {
        Some(pos) => pos,
        None => return Ok(None), // need more bytes for header
    };

    let header_line = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let parts: Vec<&str> = header_line.split_whitespace().collect();
    if parts.is_empty() {
        return Err(SmpError::ProtocolError("Empty response header".to_string()));
    }

    match parts[0] {
        "OK" => {
            let resp = if parts.len() == 1 {
                SmpResponse::Ok
            } else if parts.len() == 2 {
                SmpResponse::MessageSent {
                    message_id: parts[1].to_string(),
                }
            } else {
                SmpResponse::QueueCreated {
                    queue_id: parts[1].to_string(),
                    auth_token: parts[2].to_string(),
                }
            };
            buf.drain(..header_end + 2);
            Ok(Some(resp))
        }
        "MSG" => {
            if parts.len() < 3 {
                return Err(SmpError::ProtocolError(format!(
                    "Malformed MSG header: {}",
                    header_line
                )));
            }
            let message_id = parts[1].to_string();
            let body_len: usize = parts[2]
                .parse()
                .map_err(|_| SmpError::ProtocolError("Invalid body length in MSG".to_string()))?;

            let total_msg_len = header_end + 2 + body_len + 2; // header + \r\n + body + \r\n
            if buf.len() < total_msg_len {
                return Ok(None); // need more body bytes
            }

            let body_start = header_end + 2;
            let payload = buf[body_start..body_start + body_len].to_vec();

            buf.drain(..total_msg_len);
            Ok(Some(SmpResponse::MessageReceived {
                message_id,
                payload,
            }))
        }
        "ERR" => {
            let code = if parts.len() > 1 {
                parts[1].to_string()
            } else {
                "UNKNOWN".to_string()
            };
            let message = if parts.len() > 2 {
                parts[2..].join(" ")
            } else {
                String::new()
            };
            buf.drain(..header_end + 2);
            Ok(Some(SmpResponse::Error { code, message }))
        }
        other => Err(SmpError::ProtocolError(format!(
            "Unrecognized response status: {}",
            other
        ))),
    }
}

/// Decodes an SMP response from a complete byte slice.
pub fn decode_response(bytes: &[u8]) -> Result<SmpResponse, SmpError> {
    let mut buf = bytes.to_vec();
    match try_decode_response_from_buf(&mut buf)? {
        Some(resp) => Ok(resp),
        None => Err(SmpError::ProtocolError("Incomplete response frame".to_string())),
    }
}

pub struct SmpRelayClient<S> {
    stream: Mutex<S>,
    rx_buf: Mutex<Vec<u8>>,
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> SmpRelayClient<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream: Mutex::new(stream),
            rx_buf: Mutex::new(Vec::with_capacity(65536)),
        }
    }

    pub async fn create_queue(&self, capacity: Option<u32>) -> Result<(String, String), SmpError> {
        let cmd = SmpCommand::New { capacity };
        let resp = self.exchange_command(&cmd).await?;
        match resp {
            SmpResponse::QueueCreated { queue_id, auth_token } => Ok((queue_id, auth_token)),
            SmpResponse::Error { code, message } => Err(SmpError::RelayError { code, message }),
            _ => Err(SmpError::ProtocolError("Unexpected response to NEW".to_string())),
        }
    }

    pub async fn set_sender_key(&self, queue_id: &str, sender_auth_key: &str) -> Result<(), SmpError> {
        let cmd = SmpCommand::SetSenderKey {
            queue_id: queue_id.to_string(),
            sender_auth_key: sender_auth_key.to_string(),
        };
        let resp = self.exchange_command(&cmd).await?;
        match resp {
            SmpResponse::Ok => Ok(()),
            SmpResponse::Error { code, message } => Err(SmpError::RelayError { code, message }),
            _ => Err(SmpError::ProtocolError("Unexpected response to SKEY".to_string())),
        }
    }

    pub async fn send_message(
        &self,
        queue_id: &str,
        corr_id: &str,
        payload: &[u8],
    ) -> Result<String, SmpError> {
        let cmd = SmpCommand::Send {
            queue_id: queue_id.to_string(),
            corr_id: corr_id.to_string(),
            payload: payload.to_vec(),
        };
        let resp = self.exchange_command(&cmd).await?;
        match resp {
            SmpResponse::MessageSent { message_id } => Ok(message_id),
            SmpResponse::Error { code, message } => Err(SmpError::RelayError { code, message }),
            _ => Err(SmpError::ProtocolError("Unexpected response to SEND".to_string())),
        }
    }

    pub async fn receive_message_and_ack(&self, queue_id: &str) -> Result<(String, Vec<u8>), SmpError> {
        let mut stream = self.stream.lock().await;
        let mut rx_buf = self.rx_buf.lock().await;

        loop {
            if let Some(resp) = try_decode_response_from_buf(&mut rx_buf)? {
                match resp {
                    SmpResponse::MessageReceived { message_id, payload } => {
                        // Immediately issue ACK to trigger server-side deletion
                        let ack_cmd = SmpCommand::Ack {
                            queue_id: queue_id.to_string(),
                            message_id: message_id.clone(),
                        };
                        let ack_wire = encode_command(&ack_cmd);
                        stream
                            .write_all(&ack_wire)
                            .await
                            .map_err(|e| SmpError::ProtocolError(e.to_string()))?;

                        // Read ACK response into buffer
                        loop {
                            if let Some(ack_resp) = try_decode_response_from_buf(&mut rx_buf)? {
                                if let SmpResponse::Error { code, message } = ack_resp {
                                    return Err(SmpError::RelayError { code, message });
                                }
                                break;
                            }
                            let mut chunk = [0u8; 4096];
                            let n = stream
                                .read(&mut chunk)
                                .await
                                .map_err(|e| SmpError::ProtocolError(e.to_string()))?;
                            if n == 0 {
                                return Err(SmpError::ConnectionClosed);
                            }
                            rx_buf.extend_from_slice(&chunk[..n]);
                        }

                        return Ok((message_id, payload));
                    }
                    _ => return Err(SmpError::ProtocolError("Expected MSG frame".to_string())),
                }
            }

            let mut chunk = [0u8; 4096];
            let n = stream
                .read(&mut chunk)
                .await
                .map_err(|e| SmpError::ProtocolError(e.to_string()))?;
            if n == 0 {
                return Err(SmpError::ConnectionClosed);
            }
            rx_buf.extend_from_slice(&chunk[..n]);
        }
    }

    async fn exchange_command(&self, cmd: &SmpCommand) -> Result<SmpResponse, SmpError> {
        let mut stream = self.stream.lock().await;
        let mut rx_buf = self.rx_buf.lock().await;

        let wire = encode_command(cmd);
        stream
            .write_all(&wire)
            .await
            .map_err(|e| SmpError::ProtocolError(e.to_string()))?;

        loop {
            if let Some(resp) = try_decode_response_from_buf(&mut rx_buf)? {
                return Ok(resp);
            }

            let mut chunk = [0u8; 4096];
            let n = stream
                .read(&mut chunk)
                .await
                .map_err(|e| SmpError::ProtocolError(e.to_string()))?;
            if n == 0 {
                return Err(SmpError::ConnectionClosed);
            }
            rx_buf.extend_from_slice(&chunk[..n]);
        }
    }
}
