use simple_signal_core::crypto::padding::pad_traffic;
use simple_signal_core::net::smp::{decode_response, encode_command, SmpCommand, SmpResponse};

#[test]
fn test_encode_and_decode_new_command() {
    let cmd = SmpCommand::New { capacity: Some(50) };
    let wire_bytes = encode_command(&cmd);
    assert_eq!(wire_bytes, b"NEW 50\r\n");

    // Server responds with OK <queue_id> <queue_auth_token>
    let resp = decode_response(b"OK q_alice_001 token_xyz\r\n").expect("decode OK response");
    match resp {
        SmpResponse::QueueCreated { queue_id, auth_token } => {
            assert_eq!(queue_id, "q_alice_001");
            assert_eq!(auth_token, "token_xyz");
        }
        _ => panic!("Expected SmpResponse::QueueCreated"),
    }
}

#[test]
fn test_encode_and_decode_skey_command() {
    let cmd = SmpCommand::SetSenderKey {
        queue_id: "q_alice_001".to_string(),
        sender_auth_key: "auth_key_bob_secret".to_string(),
    };
    let wire_bytes = encode_command(&cmd);
    assert_eq!(wire_bytes, b"SKEY q_alice_001 auth_key_bob_secret\r\n");

    let resp = decode_response(b"OK\r\n").expect("decode OK response");
    assert_eq!(resp, SmpResponse::Ok);
}

#[test]
fn test_encode_and_decode_send_16kb_payload() {
    // 16KB padded ciphertext payload
    let plain = b"Secret Signal Ratchet Ciphertext";
    let padded_frame = pad_traffic(plain).expect("pad 16KB");

    let cmd = SmpCommand::Send {
        queue_id: "q_bob_recv".to_string(),
        corr_id: "corr_123".to_string(),
        payload: padded_frame.to_vec(),
    };

    let wire_bytes = encode_command(&cmd);
    assert!(wire_bytes.starts_with(b"SEND q_bob_recv corr_123 16384\r\n"));
    assert!(wire_bytes.ends_with(b"\r\n"));
    assert_eq!(wire_bytes.len(), b"SEND q_bob_recv corr_123 16384\r\n".len() + 16384 + 2);

    let resp = decode_response(b"OK msg_98765\r\n").expect("decode send OK");
    match resp {
        SmpResponse::MessageSent { message_id } => {
            assert_eq!(message_id, "msg_98765");
        }
        _ => panic!("Expected SmpResponse::MessageSent"),
    }
}

#[test]
fn test_encode_and_decode_msg_and_ack() {
    // Incoming MSG frame from server
    let plain = b"Incoming message body";
    let padded_frame = pad_traffic(plain).expect("pad 16KB");
    
    let mut server_frame = Vec::new();
    server_frame.extend_from_slice(b"MSG msg_555 16384\r\n");
    server_frame.extend_from_slice(&padded_frame);
    server_frame.extend_from_slice(b"\r\n");

    let resp = decode_response(&server_frame).expect("decode MSG response");
    match resp {
        SmpResponse::MessageReceived { message_id, payload } => {
            assert_eq!(message_id, "msg_555");
            assert_eq!(payload.len(), 16384);
        }
        _ => panic!("Expected SmpResponse::MessageReceived"),
    }

    // Client sends ACK to instruct server to immediately wipe message
    let ack_cmd = SmpCommand::Ack {
        queue_id: "q_alice_001".to_string(),
        message_id: "msg_555".to_string(),
    };
    let ack_wire = encode_command(&ack_cmd);
    assert_eq!(ack_wire, b"ACK q_alice_001 msg_555\r\n");
}

#[test]
fn test_decode_error_response() {
    let err_resp = decode_response(b"ERR QUEUE_NOT_FOUND\r\n").expect("decode ERR");
    match err_resp {
        SmpResponse::Error { code, message } => {
            assert_eq!(code, "QUEUE_NOT_FOUND");
            assert!(message.is_empty());
        }
        _ => panic!("Expected SmpResponse::Error"),
    }
}
