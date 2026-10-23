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
    })
    .expect("insert msg1");

    db.insert_message(&MessageRecord {
        id: "msg-2".to_string(),
        conversation_id: "conv-alice-bob".to_string(),
        direction: "inbound".to_string(),
        body: b"Hey Alice!".to_vec(),
        timestamp: 1700000025,
        status: "received".to_string(),
    })
    .expect("insert msg2");

    db.insert_message(&MessageRecord {
        id: "msg-3".to_string(),
        conversation_id: "conv-alice-charlie".to_string(),
        direction: "outbound".to_string(),
        body: b"Hey Charlie".to_vec(),
        timestamp: 1700000030,
        status: "sent".to_string(),
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
