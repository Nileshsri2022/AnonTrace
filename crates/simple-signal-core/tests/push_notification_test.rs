use simple_signal_core::net::push::{
    blind_device_token, EphemeralWakeupCoordinator, PushCommand, PushNotificationGateway,
    PushResponse, PushService,
};
use simple_signal_core::net::tor::TorClientManager;
use tokio::io::duplex;

#[test]
fn test_push_wire_codec_roundtrip() {
    // 1. TNEW (Register mobile device token with notification router)
    let cmd_tnew = PushCommand::RegisterToken {
        service: PushService::Apns,
        device_token: "abcd1234efgh5678".to_string(),
    };
    let wire_tnew = cmd_tnew.encode();
    assert_eq!(wire_tnew, "TNEW apns abcd1234efgh5678\r\n");

    let resp_tnew = PushResponse::parse("TNEW bnd_tok_99887766\r\n").expect("parse TNEW resp");
    assert_eq!(
        resp_tnew,
        PushResponse::TokenRegistered {
            blinded_token: "bnd_tok_99887766".to_string()
        }
    );

    // 2. NKEY (Bind notifier key on SMP message relay)
    let cmd_nkey = PushCommand::BindNotifier {
        queue_id: "q_alice_123".to_string(),
        blinded_token: "bnd_tok_99887766".to_string(),
    };
    let wire_nkey = cmd_nkey.encode();
    assert_eq!(wire_nkey, "NKEY q_alice_123 bnd_tok_99887766\r\n");

    let resp_nkey = PushResponse::parse("OK\r\n").expect("parse NKEY resp");
    assert_eq!(resp_nkey, PushResponse::Ok);

    // 3. SNEW (Subscribe queue notifier with Notification Router)
    let cmd_snew = PushCommand::SubscribePush {
        blinded_token: "bnd_tok_99887766".to_string(),
        queue_tag: "tag_q_alice_456".to_string(),
    };
    let wire_snew = cmd_snew.encode();
    assert_eq!(wire_snew, "SNEW bnd_tok_99887766 tag_q_alice_456\r\n");

    let resp_snew = PushResponse::parse("SNEW OK\r\n").expect("parse SNEW resp");
    assert_eq!(resp_snew, PushResponse::Subscribed);

    // 4. NMSG (Wakeup trigger emitted on message arrival)
    let cmd_nmsg = PushCommand::TriggerPush {
        queue_tag: "tag_q_alice_456".to_string(),
    };
    let wire_nmsg = cmd_nmsg.encode();
    assert_eq!(wire_nmsg, "NMSG tag_q_alice_456\r\n");

    let resp_nmsg = PushResponse::parse("NMSG tag_q_alice_456\r\n").expect("parse NMSG resp");
    assert_eq!(
        resp_nmsg,
        PushResponse::NotificationTriggered {
            queue_tag: "tag_q_alice_456".to_string()
        }
    );

    // 5. Error response parsing
    let resp_err = PushResponse::parse("ERR INVALID_TOKEN Token not found\r\n").expect("parse ERR");
    assert_eq!(
        resp_err,
        PushResponse::Error {
            code: "INVALID_TOKEN".to_string(),
            message: "Token not found".to_string()
        }
    );
}

#[test]
fn test_blinded_token_deterministic_and_unlinkable() {
    let raw_token = "fcm_device_token_xyz_987654321";
    let salt = [0x42u8; 16];

    let blinded1 = blind_device_token(raw_token, &salt);
    let blinded2 = blind_device_token(raw_token, &salt);

    // Deterministic with same salt
    assert_eq!(blinded1, blinded2);
    assert_ne!(blinded1, raw_token);
    assert_eq!(blinded1.len(), 64); // 32-byte hex hash

    // Different salt yields completely distinct token
    let salt2 = [0x43u8; 16];
    let blinded3 = blind_device_token(raw_token, &salt2);
    assert_ne!(blinded1, blinded3);
}

#[tokio::test]
async fn test_push_notification_gateway_registration_flow() {
    let (mut client_stream, mut server_stream) = duplex(1024);

    let server_task = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut buf = vec![0u8; 1024];
        let n = server_stream.read(&mut buf).await.unwrap();
        let received = String::from_utf8_lossy(&buf[..n]);
        assert_eq!(received, "TNEW fcm fcm_token_12345\r\n");

        server_stream.write_all(b"TNEW bnd_fcm_xyz99\r\n").await.unwrap();
    });

    let gateway = PushNotificationGateway::new();
    let blinded_token = gateway
        .register_device_token(&mut client_stream, PushService::Fcm, "fcm_token_12345")
        .await
        .expect("register token");

    assert_eq!(blinded_token, "bnd_fcm_xyz99");
    server_task.await.unwrap();
}

#[tokio::test]
async fn test_ephemeral_wakeup_retrieval_cycle() {
    let tor_manager = TorClientManager::default_client();
    assert_eq!(tor_manager.is_active(), true);

    let coordinator = EphemeralWakeupCoordinator::new(tor_manager.clone());

    // Enter battery-saving dormancy
    coordinator.enter_dormancy().await.expect("enter dormancy");
    assert_eq!(tor_manager.is_active(), false);

    // Push wakeup event arrives
    let wakeup_result = coordinator
        .handle_push_wakeup("tag_q_alice_456")
        .await
        .expect("handle push wakeup");

    assert_eq!(wakeup_result.queue_tag, "tag_q_alice_456");
    assert_eq!(wakeup_result.retrieval_executed, true);

    // Coordinator automatically returns to battery-saving dormancy after retrieval
    assert_eq!(tor_manager.is_active(), false);
}
