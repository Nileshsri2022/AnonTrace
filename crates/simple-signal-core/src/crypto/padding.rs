use thiserror::Error;

pub const TRAFFIC_FRAME_SIZE: usize = 16384;
pub const MAX_PAYLOAD_SIZE: usize = TRAFFIC_FRAME_SIZE - 2; // 2 bytes for big-endian length

#[derive(Error, Debug, PartialEq, Eq)]
pub enum CryptoError {
    #[error("Payload size {size} exceeds maximum allowed {max} bytes for 16KB frame")]
    PayloadTooLarge { size: usize, max: usize },

    #[error("Invalid traffic padding: {0}")]
    InvalidPadding(String),

    #[error("Post-quantum key exchange failed: {0}")]
    KeyExchangeFailed(String),

    #[error("Double Ratchet decryption failed: {0}")]
    DecryptionFailed(String),
}

/// Pads an arbitrary payload up to 16,382 bytes into an exact 16,384-byte traffic-masked frame.
pub fn pad_traffic(payload: &[u8]) -> Result<[u8; TRAFFIC_FRAME_SIZE], CryptoError> {
    if payload.len() > MAX_PAYLOAD_SIZE {
        return Err(CryptoError::PayloadTooLarge {
            size: payload.len(),
            max: MAX_PAYLOAD_SIZE,
        });
    }

    let mut frame = [0u8; TRAFFIC_FRAME_SIZE];

    // Write 2-byte big-endian length prefix
    let len_bytes = (payload.len() as u16).to_be_bytes();
    frame[0] = len_bytes[0];
    frame[1] = len_bytes[1];

    // Copy payload
    frame[2..2 + payload.len()].copy_from_slice(payload);

    // Remaining bytes are filled with deterministic traffic masking '#' (0x23) per SimpleX convention
    for byte in &mut frame[2 + payload.len()..] {
        *byte = b'#';
    }

    Ok(frame)
}

/// Extracts the original payload from a 16,384-byte traffic frame.
pub fn unpad_traffic(frame: &[u8; TRAFFIC_FRAME_SIZE]) -> Result<Vec<u8>, CryptoError> {
    let len = u16::from_be_bytes([frame[0], frame[1]]) as usize;

    if len > MAX_PAYLOAD_SIZE {
        return Err(CryptoError::InvalidPadding(format!(
            "Payload length {} exceeds maximum allowed {}",
            len, MAX_PAYLOAD_SIZE
        )));
    }

    Ok(frame[2..2 + len].to_vec())
}
