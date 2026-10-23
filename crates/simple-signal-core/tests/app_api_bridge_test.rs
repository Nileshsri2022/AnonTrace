use simple_signal_core::api::{FfiError, SimpleSignalApp};
use tempfile::tempdir;

#[test]
fn test_app_api_initialization_and_wrong_passphrase_rejection() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("secure_test.db");
    let db_str = db_path.to_str().unwrap();

    // 1. Initialize first time with valid passphrase
    let app = SimpleSignalApp::init(db_str, "correct_master_pin_1234").expect("init app");
    assert_eq!(app.is_active(), true);
    drop(app);

    // 2. Re-open with incorrect passphrase -> MUST fail with AuthenticationFailed
    let err = SimpleSignalApp::init(db_str, "wrong_master_pin_9999")
        .expect_err("should reject wrong master pin");
    match err {
        FfiError::AuthenticationFailed => {}
        other => panic!("Expected FfiError::AuthenticationFailed, got {:?}", other),
    }

    // 3. Re-open with correct passphrase -> succeeds
    let reopened =
        SimpleSignalApp::init(db_str, "correct_master_pin_1234").expect("reopen app");
    assert_eq!(reopened.is_active(), true);
}

#[test]
fn test_app_api_contact_exchange_and_messaging_flow() {
    let dir = tempdir().expect("tempdir");
    let alice_db = dir.path().join("alice.db");
    let bob_db = dir.path().join("bob.db");

    let alice_app =
        SimpleSignalApp::init(alice_db.to_str().unwrap(), "alice_pin_1111").expect("alice init");
    let bob_app =
        SimpleSignalApp::init(bob_db.to_str().unwrap(), "bob_pin_2222").expect("bob init");

    // 1. Alice creates her Contact Link (for QR display)
    let alice_link_uri = alice_app.get_my_contact_link().expect("alice contact link");
    assert!(alice_link_uri.starts_with("simplex:/contact#/?"));

    // 2. Bob imports / scans Alice's Contact Link
    let conv = bob_app
        .import_contact_link(&alice_link_uri)
        .expect("bob import alice link");
    assert!(!conv.id.is_empty());
    assert!(conv.send_queue_uri.starts_with("simplex://"));

    // 3. Bob lists conversations -> Alice's conversation exists
    let conversations = bob_app.list_conversations().expect("list conversations");
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0].id, conv.id);

    // 4. Bob sends a chat message
    let msg = bob_app
        .send_chat_message(&conv.id, "Hello from Flutter UI over Simple+Signal core!")
        .expect("send message");
    assert_eq!(msg.direction, "outbound");
    assert_eq!(msg.body_text, "Hello from Flutter UI over Simple+Signal core!");
    assert_eq!(msg.status, "sent");

    // 5. Bob queries messages for this conversation
    let messages = bob_app.list_messages(&conv.id).expect("list messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].id, msg.id);
    assert_eq!(messages[0].body_text, msg.body_text);

    // 6. Dormancy toggle
    bob_app.set_dormant(true).expect("set dormant true");
    assert_eq!(bob_app.is_active(), false);

    bob_app.set_dormant(false).expect("set dormant false");
    assert_eq!(bob_app.is_active(), true);
}

#[test]
fn test_reimporting_contact_is_idempotent_and_session_survives_restart() {
    let dir = tempdir().expect("tempdir");
    let alice_db = dir.path().join("alice.db");
    let bob_db = dir.path().join("bob.db");

    let alice_app =
        SimpleSignalApp::init(alice_db.to_str().unwrap(), "alice_pin_1111").expect("alice init");
    let bob_app =
        SimpleSignalApp::init(bob_db.to_str().unwrap(), "bob_pin_2222").expect("bob init");

    let alice_link_uri = alice_app.get_my_contact_link().expect("alice contact link");

    // 1. Bob imports Alice's link
    let conv1 = bob_app.import_contact_link(&alice_link_uri).expect("first import");

    // 2. Bob imports Alice's link AGAIN (simulating duplicate camera scan or re-scan)
    // BEFORE FIX: Fails with "UNIQUE constraint failed: conversations.id"
    let conv2 = bob_app
        .import_contact_link(&alice_link_uri)
        .expect("second import should be idempotent");
    assert_eq!(conv1.id, conv2.id);

    // 3. Drop bob_app (simulating app process kill or restart)
    drop(bob_app);

    // 4. Bob reopens app with PIN
    let reopened_bob =
        SimpleSignalApp::init(bob_db.to_str().unwrap(), "bob_pin_2222").expect("bob reopen");

    // 5. Bob sends message to Alice's conversation
    // BEFORE FIX: Fails with "Item not found: Active session not found"
    let msg = reopened_bob
        .send_chat_message(&conv1.id, "Hello after app restart!")
        .expect("send after restart");
    assert_eq!(msg.body_text, "Hello after app restart!");
}
