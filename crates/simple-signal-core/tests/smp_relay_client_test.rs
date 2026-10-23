use simple_signal_core::net::smp::SmpRelayClient;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn test_smp_client_send_and_receive_with_auto_ack() {
    // Create an in-memory duplex stream (client <-> mock relay)
    let (client_stream, mut mock_relay_stream) = duplex(65536);

    let client = SmpRelayClient::new(client_stream);

    // Spawn mock SMP relay broker task
    tokio::spawn(async move {
        let mut buf = vec![0u8; 32768];

        // 1. Mock relay expects NEW command
        let n = mock_relay_stream.read(&mut buf).await.expect("read NEW");
        assert!(buf[..n].starts_with(b"NEW"));
        mock_relay_stream
            .write_all(b"OK q_test_999 auth_token_abc\r\n")
            .await
            .expect("write OK NEW");

        // 2. Mock relay expects SKEY command
        let n = mock_relay_stream.read(&mut buf).await.expect("read SKEY");
        assert!(buf[..n].starts_with(b"SKEY q_test_999"));
        mock_relay_stream.write_all(b"OK\r\n").await.expect("write OK SKEY");

        // 3. Mock relay receives SEND command (read header + 16KB payload)
        let mut send_buf = vec![0u8; 20000];
        let mut total_read = 0;
        while total_read < b"SEND q_test_999 corr_55 16384\r\n".len() + 16384 + 2 {
            let n = mock_relay_stream.read(&mut send_buf[total_read..]).await.expect("read SEND");
            if n == 0 { break; }
            total_read += n;
        }
        assert!(send_buf[..total_read].starts_with(b"SEND q_test_999"));
        mock_relay_stream
            .write_all(b"OK msg_relay_101\r\n")
            .await
            .expect("write OK SEND");

        // 4. Mock relay pushes an incoming MSG frame to subscriber
        let mut push_frame = Vec::new();
        push_frame.extend_from_slice(b"MSG msg_relay_102 16384\r\n");
        push_frame.extend_from_slice(&vec![0x42; 16384]);
        push_frame.extend_from_slice(b"\r\n");
        mock_relay_stream.write_all(&push_frame).await.expect("push MSG");

        // 5. Mock relay expects client to immediately send ACK
        let n = mock_relay_stream.read(&mut buf).await.expect("read ACK");
        assert_eq!(&buf[..n], b"ACK q_test_999 msg_relay_102\r\n");
        mock_relay_stream.write_all(b"OK\r\n").await.expect("write OK ACK");
    });

    // Act 1: Allocate queue
    let (queue_id, auth_token) = client.create_queue(Some(100)).await.expect("create queue");
    assert_eq!(queue_id, "q_test_999");
    assert_eq!(auth_token, "auth_token_abc");

    // Act 2: Set sender key
    client
        .set_sender_key(&queue_id, "secret_sender_key")
        .await
        .expect("set sender key");

    // Act 3: Drop 16KB payload
    let test_payload = vec![0x11; 16384];
    let msg_id = client
        .send_message(&queue_id, "corr_55", &test_payload)
        .await
        .expect("send message");
    assert_eq!(msg_id, "msg_relay_101");

    // Act 4: Receive incoming message and verify auto-ACK
    let (incoming_msg_id, incoming_payload) = client
        .receive_message_and_ack(&queue_id)
        .await
        .expect("receive and ack");
    assert_eq!(incoming_msg_id, "msg_relay_102");
    assert_eq!(incoming_payload.len(), 16384);
}
