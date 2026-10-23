use simple_signal_core::net::smp::{QueueStatus, SmpError, SmpQueueSession};

#[test]
fn test_queue_session_lifecycle_transitions() {
    let mut session = SmpQueueSession::new();
    assert_eq!(session.status(), QueueStatus::Uninitialized);

    // Cannot configure sender key on uninitialized queue
    let err = session.configure_sender_key("auth_key_123").expect_err("should fail");
    match err {
        SmpError::ProtocolError(msg) => assert!(msg.contains("Queue not allocated")),
        _ => panic!("Expected SmpError::ProtocolError"),
    }

    // Allocate queue via NEW response
    session
        .on_queue_allocated("q_rec_456", "token_789")
        .expect("allocate queue");
    assert_eq!(
        session.status(),
        QueueStatus::Allocated {
            queue_id: "q_rec_456".to_string(),
            auth_token: "token_789".to_string(),
        }
    );

    // Now secure queue with sender key
    session
        .configure_sender_key("auth_key_bob_xyz")
        .expect("secure queue");
    assert_eq!(
        session.status(),
        QueueStatus::Secured {
            queue_id: "q_rec_456".to_string(),
            sender_auth_key: "auth_key_bob_xyz".to_string(),
        }
    );

    // Queue ID is accessible
    assert_eq!(session.queue_id().unwrap(), "q_rec_456");
}
