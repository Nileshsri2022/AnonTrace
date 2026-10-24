use simple_signal_core::storage::db::{
    ConversationRecord, EncryptedDatabase, MessageRecord, SessionRecord,
};
use tempfile::tempdir;

#[test]
fn test_deleting_conversation_triggers_atomic_cascading_delete() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("test_cascade.db");
    let key = [0x77u8; 32];

    let db = EncryptedDatabase::open_or_create(&db_path, &key)
        .expect("open or create database");

    // Insert Target Conversation
    let conv1 = ConversationRecord {
        id: "conv-alice-bob".to_string(),
        send_queue_uri: "smp://onion1.onion/q1".to_string(),
        receive_queue_id: "recv-q-1".to_string(),
        relay_onion_address: "relay1.onion".to_string(),
        sender_auth_key: vec![0x11; 32],
        created_at: 1700000000,
    };
    db.insert_conversation(&conv1).expect("insert conv1");

    // Insert Unrelated Conversation
    let conv2 = ConversationRecord {
        id: "conv-alice-charlie".to_string(),
        send_queue_uri: "smp://onion2.onion/q2".to_string(),
        receive_queue_id: "recv-q-2".to_string(),
        relay_onion_address: "relay2.onion".to_string(),
        sender_auth_key: vec![0x22; 32],
        created_at: 1700000001,
    };
    db.insert_conversation(&conv2).expect("insert conv2");

    // Insert Sessions
    db.insert_session(&SessionRecord {
        conversation_id: "conv-alice-bob".to_string(),
        session_record: vec![0xAA, 0xBB, 0xCC],
        updated_at: 1700000010,
    })
    .expect("insert session for conv1");

    db.insert_session(&SessionRecord {
        conversation_id: "conv-alice-charlie".to_string(),
        session_record: vec![0xDD, 0xEE, 0xFF],
        updated_at: 1700000011,
    })
    .expect("insert session for conv2");

    // Insert Messages
    db.insert_message(&MessageRecord {
        id: "msg-1".to_string(),
        conversation_id: "conv-alice-bob".to_string(),
        direction: "outbound".to_string(),
        body: b"Hello Bob!".to_vec(),
        timestamp: 1700000020,
        status: "delivered".to_string(),
        expires_at: None,
    })
    .expect("insert msg1");

    db.insert_message(&MessageRecord {
        id: "msg-2".to_string(),
        conversation_id: "conv-alice-bob".to_string(),
        direction: "inbound".to_string(),
        body: b"Hey Alice!".to_vec(),
        timestamp: 1700000025,
        status: "received".to_string(),
        expires_at: None,
    })
    .expect("insert msg2");

    db.insert_message(&MessageRecord {
        id: "msg-3".to_string(),
        conversation_id: "conv-alice-charlie".to_string(),
        direction: "outbound".to_string(),
        body: b"Hey Charlie".to_vec(),
        timestamp: 1700000030,
        status: "sent".to_string(),
        expires_at: None,
    })
    .expect("insert msg3");

    // Verify initial counts for conv-alice-bob
    assert_eq!(db.count_sessions("conv-alice-bob").expect("count sessions"), 1);
    assert_eq!(db.count_messages("conv-alice-bob").expect("count messages"), 2);

    // ACT: Delete conv-alice-bob
    let deleted = db.delete_conversation("conv-alice-bob").expect("delete conversation");
    assert!(deleted, "Expected true indicating conversation existed and was deleted");

    // ASSERT: All conv-alice-bob records must be completely gone (cascading delete)
    assert!(db.get_conversation("conv-alice-bob").expect("query conv").is_none());
    assert_eq!(db.count_sessions("conv-alice-bob").expect("count sessions"), 0);
    assert_eq!(db.count_messages("conv-alice-bob").expect("count messages"), 0);

    // ASSERT: Unrelated conversation and its records must be completely intact
    assert!(db.get_conversation("conv-alice-charlie").expect("query conv2").is_some());
    assert_eq!(db.count_sessions("conv-alice-charlie").expect("count sessions"), 1);
    assert_eq!(db.count_messages("conv-alice-charlie").expect("count messages"), 1);
}

#[test]
fn test_delete_individual_message() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("test_delete_msg.db");
    let key = [0x77u8; 32];
    let db = EncryptedDatabase::open_or_create(&db_path, &key).expect("open db");

    let conv = ConversationRecord {
        id: "c1".to_string(),
        send_queue_uri: "smp://onion1.onion/q1".to_string(),
        receive_queue_id: "recv-q-1".to_string(),
        relay_onion_address: "relay1.onion".to_string(),
        sender_auth_key: vec![0x11; 32],
        created_at: 1700000000,
    };
    db.insert_conversation(&conv).expect("insert conv");

    let msg1 = MessageRecord {
        id: "msg-1".to_string(),
        conversation_id: "c1".to_string(),
        direction: "outbound".to_string(),
        body: b"Hello".to_vec(),
        timestamp: 100,
        status: "sent".to_string(),
        expires_at: None,
    };
    let msg2 = MessageRecord {
        id: "msg-2".to_string(),
        conversation_id: "c1".to_string(),
        direction: "inbound".to_string(),
        body: b"World".to_vec(),
        timestamp: 101,
        status: "received".to_string(),
        expires_at: None,
    };
    db.insert_message(&msg1).expect("insert msg1");
    db.insert_message(&msg2).expect("insert msg2");

    assert_eq!(db.count_messages("c1").expect("count"), 2);

    let deleted = db.delete_message("msg-1").expect("delete msg-1");
    assert!(deleted);

    assert_eq!(db.count_messages("c1").expect("count"), 1);
    let msgs = db.list_messages("c1").expect("list");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].id, "msg-2");

    let not_deleted = db.delete_message("msg-999").expect("delete non-existent");
    assert!(!not_deleted);
}

#[test]
fn test_update_message_status() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("test_status.db");
    let key = [0x77u8; 32];
    let db = EncryptedDatabase::open_or_create(&db_path, &key).expect("open db");

    let conv = ConversationRecord {
        id: "c1".to_string(),
        send_queue_uri: "smp://onion1.onion/q1".to_string(),
        receive_queue_id: "recv-q-1".to_string(),
        relay_onion_address: "relay1.onion".to_string(),
        sender_auth_key: vec![0x11; 32],
        created_at: 1700000000,
    };
    db.insert_conversation(&conv).expect("insert conv");

    let msg1 = MessageRecord {
        id: "msg-status-1".to_string(),
        conversation_id: "c1".to_string(),
        direction: "outbound".to_string(),
        body: b"Hello".to_vec(),
        timestamp: 100,
        status: "sent".to_string(),
        expires_at: None,
    };
    db.insert_message(&msg1).expect("insert msg1");

    let msgs = db.list_messages("c1").expect("list");
    assert_eq!(msgs[0].status, "sent");

    let updated = db.update_message_status("msg-status-1", "delivered").expect("update");
    assert!(updated);

    let msgs2 = db.list_messages("c1").expect("list");
    assert_eq!(msgs2[0].status, "delivered");

    let updated_read = db.update_message_status("msg-status-1", "read").expect("update");
    assert!(updated_read);

    let msgs3 = db.list_messages("c1").expect("list");
    assert_eq!(msgs3[0].status, "read");
}

#[test]
fn test_disappearing_messages_ttl_and_purge() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("test_disappearing.db");
    let key = [0x77u8; 32];
    let db = EncryptedDatabase::open_or_create(&db_path, &key).expect("open db");

    let conv = ConversationRecord {
        id: "c_ephemeral".to_string(),
        send_queue_uri: "smp://onion1.onion/q1".to_string(),
        receive_queue_id: "recv-q-1".to_string(),
        relay_onion_address: "relay1.onion".to_string(),
        sender_auth_key: vec![0x11; 32],
        created_at: 1000,
    };
    db.insert_conversation(&conv).expect("insert conv");

    // Set 60-second disappearing timer
    db.set_disappearing_timer("c_ephemeral", 60).expect("set timer");
    assert_eq!(db.get_disappearing_timer("c_ephemeral").expect("get timer"), 60);

    // Message 1: expires at t=1060
    let msg1 = MessageRecord {
        id: "m_expiring".to_string(),
        conversation_id: "c_ephemeral".to_string(),
        direction: "outbound".to_string(),
        body: b"Self destructing in 60s".to_vec(),
        timestamp: 1000,
        status: "sent".to_string(),
        expires_at: Some(1060),
    };
    db.insert_message(&msg1).expect("insert msg1");

    // Message 2: permanent (expires_at = None)
    let msg2 = MessageRecord {
        id: "m_permanent".to_string(),
        conversation_id: "c_ephemeral".to_string(),
        direction: "inbound".to_string(),
        body: b"Keep forever".to_vec(),
        timestamp: 1001,
        status: "received".to_string(),
        expires_at: None,
    };
    db.insert_message(&msg2).expect("insert msg2");

    assert_eq!(db.list_messages("c_ephemeral").expect("list").len(), 2);

    // At t=1050 (before expiration): purge removes 0 messages
    let purged_early = db.purge_expired_messages(1050).expect("purge early");
    assert_eq!(purged_early, 0);
    assert_eq!(db.list_messages("c_ephemeral").expect("list").len(), 2);

    // At t=1061 (after expiration): purge removes 1 message
    let purged_late = db.purge_expired_messages(1061).expect("purge late");
    assert_eq!(purged_late, 1);

    let remaining = db.list_messages("c_ephemeral").expect("list");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, "m_permanent");
}
