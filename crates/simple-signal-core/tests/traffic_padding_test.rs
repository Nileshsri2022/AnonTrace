use simple_signal_core::crypto::padding::{pad_traffic, unpad_traffic, CryptoError, TRAFFIC_FRAME_SIZE};

#[test]
fn test_traffic_padding_roundtrip_and_fixed_16kb_size() {
    let payload = b"Hello, post-quantum anonymous world over Tor!";
    
    // 1. Pad payload to exactly 16,384 bytes
    let frame = pad_traffic(payload).expect("padding should succeed");
    assert_eq!(frame.len(), TRAFFIC_FRAME_SIZE);
    assert_eq!(frame.len(), 16384);

    // 2. Unpad frame recovers identical original payload
    let recovered = unpad_traffic(&frame).expect("unpadding should succeed");
    assert_eq!(recovered, payload);
}

#[test]
fn test_traffic_padding_rejects_oversized_payload() {
    // Max allowed payload size is 16,384 - 2 = 16,382 bytes
    let max_payload = vec![0xAA; 16382];
    let frame = pad_traffic(&max_payload).expect("max payload should succeed");
    assert_eq!(frame.len(), 16384);

    let recovered = unpad_traffic(&frame).expect("unpadding max payload");
    assert_eq!(recovered, max_payload);

    // Oversized payload (16,383 bytes) must fail
    let oversized = vec![0xBB; 16383];
    let err = pad_traffic(&oversized).expect_err("oversized payload must fail");
    match err {
        CryptoError::PayloadTooLarge { size, max } => {
            assert_eq!(size, 16383);
            assert_eq!(max, 16382);
        }
        _ => panic!("Expected CryptoError::PayloadTooLarge"),
    }
}

#[test]
fn test_unpad_rejects_corrupted_length_prefix() {
    let mut corrupt_frame = [0u8; 16384];
    // Put length prefix 0xFFFF (65535), which exceeds 16382
    corrupt_frame[0] = 0xFF;
    corrupt_frame[1] = 0xFF;

    let err = unpad_traffic(&corrupt_frame).expect_err("corrupted length prefix must fail");
    match err {
        CryptoError::InvalidPadding(msg) => {
            assert!(msg.contains("exceeds maximum allowed"));
        }
        _ => panic!("Expected CryptoError::InvalidPadding"),
    }
}
