use simple_signal_core::api::SimpleSignalApp;
use simple_signal_core::storage::db::{
    compute_auth_verifier, derive_key, EncryptedDatabase, KeyDerivationParams, MessageRecord,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_identity_and_contact_link_survive_restart() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("alice.db");
    let db_str = db_path.to_str().unwrap();

    let app = SimpleSignalApp::init(db_str, "alice_pin_1111").expect("first init");
    let link1 = app.get_my_contact_link().expect("link1");
    let queue1 = app.my_inbox_queue_id();
    drop(app);

    let app2 = SimpleSignalApp::init(db_str, "alice_pin_1111").expect("second init");
    let link2 = app2.get_my_contact_link().expect("link2");
    let queue2 = app2.my_inbox_queue_id();
    assert_eq!(link1, link2, "Contact link must be identical after restart");
    assert_eq!(queue1, queue2, "Inbox queue must be identical after restart");
}

#[test]
fn test_handshake_and_message_work_after_restart_with_persisted_prekeys() {
    // Exercises the Kyber decapsulation secret: if it were regenerated
    // on restart, Alice could not accept Bob's handshake built from
    // her pre-restart bundle.
    let dir = tempdir().expect("tempdir");
    let alice_db = dir.path().join("alice.db");
    let bob_db = dir.path().join("bob.db");

    let alice_app =
        SimpleSignalApp::init(alice_db.to_str().unwrap(), "alice_pin").expect("alice init");
    let alice_link = alice_app.get_my_contact_link().expect("alice link");
    drop(alice_app);

    let alice_app =
        SimpleSignalApp::init(alice_db.to_str().unwrap(), "alice_pin").expect("alice re-init");
    let bob_app = SimpleSignalApp::init(bob_db.to_str().unwrap(), "bob_pin").expect("bob init");
    assert_eq!(
        alice_link,
        alice_app.get_my_contact_link().expect("alice link after restart")
    );

    let (bob_conv, _, handshake_hex) = bob_app
        .import_contact_link_with_handshake(&alice_link)
        .expect("bob import");
    let handshake_bytes = hex::decode(&handshake_hex).expect("decode handshake");
    alice_app
        .process_incoming_payload(&handshake_bytes)
        .expect("alice accept handshake");

    let (bob_msg, _, payload_hex) = bob_app
        .prepare_outbound_chat_message(&bob_conv.id, "Hello after Alice restart!")
        .expect("bob prepare");
    assert_eq!(bob_msg.body_text, "Hello after Alice restart!");
    let payload_bytes = hex::decode(&payload_hex).expect("decode payload");
    let decrypted = alice_app
        .process_incoming_payload(&payload_bytes)
        .expect("alice decrypt")
        .expect("alice message");
    assert_eq!(decrypted.body_text, "Hello after Alice restart!");
}

#[test]
fn test_salt_is_random_per_database() {
    let dir = tempdir().expect("tempdir");
    let db1 = dir.path().join("one.db");
    let db2 = dir.path().join("two.db");
    let app1 = SimpleSignalApp::init(db1.to_str().unwrap(), "same_pin").expect("init one");
    let app2 = SimpleSignalApp::init(db2.to_str().unwrap(), "same_pin").expect("init two");
    drop(app1);
    drop(app2);
    let salt1 = fs::read_to_string(format!("{}.salt", db1.to_str().unwrap())).expect("salt1");
    let salt2 = fs::read_to_string(format!("{}.salt", db2.to_str().unwrap())).expect("salt2");
    assert_eq!(salt1.len(), 32, "Salt file must be 16 bytes as hex");
    assert_ne!(salt1, salt2, "Different databases must get different salts");
}

#[test]
fn test_db_file_is_encrypted_at_rest() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("at_rest.db");
    let params = KeyDerivationParams {
        memory_kib: 8 * 1024,
        iterations: 1,
        parallelism: 1,
        salt: [0x42; 16],
    };
    let key = derive_key("test-pin", &params).expect("derive key");
    let secret = "unique_at_rest_secret_xyz_12345";
    {
        let db = EncryptedDatabase::open_or_create(&db_path, &key).expect("open db");
        db.insert_conversation(&simple_signal_core::storage::db::ConversationRecord {
            id: "conv_at_rest".to_string(),
            send_queue_uri: "simplex://example/queue".to_string(),
            receive_queue_id: "queue1".to_string(),
            relay_onion_address: "example.onion".to_string(),
            sender_auth_key: vec![0x11; 32],
            created_at: 1,
        })
        .expect("insert conv");
        db.insert_message(&MessageRecord {
            id: "msg_at_rest".to_string(),
            conversation_id: "conv_at_rest".to_string(),
            direction: "outbound".to_string(),
            body: secret.as_bytes().to_vec(),
            timestamp: 1,
            status: "sent".to_string(),
            expires_at: None,
        })
        .expect("insert msg");
    }
    let raw = fs::read(&db_path).expect("read db file");
    assert_ne!(
        &raw[0..16],
        b"SQLite format 3\0",
        "File header must not be a plaintext SQLite header"
    );
    assert!(
        !raw.windows(secret.len())
            .any(|w| w == secret.as_bytes()),
        "Plaintext message must not appear in the DB file"
    );
}

#[test]
fn test_legacy_plaintext_db_migrates_and_preserves_chats() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("legacy.db");
    let db_str = db_path.to_str().unwrap();
    let pin = "legacy_pin_1234";

    // Build a pre-Option-A database by hand: plaintext file, constant
    // salt, HMAC verifier, one conversation + one message, no salt file.
    {
        let legacy_params = KeyDerivationParams {
            memory_kib: 8 * 1024,
            iterations: 1,
            parallelism: 1,
            salt: [0x77; 16],
        };
        let legacy_key = derive_key(pin, &legacy_params).expect("legacy derive");
        let verifier = compute_auth_verifier(&legacy_key);
        let conn = rusqlite::Connection::open(&db_path).expect("legacy open");
        conn.execute_batch(
            "CREATE TABLE _security_header (id INTEGER PRIMARY KEY CHECK (id = 1), auth_verifier BLOB NOT NULL);
             CREATE TABLE conversations (id TEXT PRIMARY KEY, send_queue_uri TEXT NOT NULL, receive_queue_id TEXT NOT NULL, relay_onion_address TEXT NOT NULL, sender_auth_key BLOB NOT NULL, created_at INTEGER NOT NULL);
             CREATE TABLE sessions (id INTEGER PRIMARY KEY AUTOINCREMENT, conversation_id TEXT NOT NULL, session_record BLOB NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE messages (id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL, direction TEXT NOT NULL, body BLOB NOT NULL, timestamp INTEGER NOT NULL, status TEXT NOT NULL, expires_at INTEGER DEFAULT NULL);",
        )
        .expect("legacy schema");
        conn.execute(
            "INSERT INTO _security_header (id, auth_verifier) VALUES (1, ?1);",
            rusqlite::params![&verifier[..]],
        )
        .expect("legacy verifier");
        conn.execute(
            "INSERT INTO conversations (id, send_queue_uri, receive_queue_id, relay_onion_address, sender_auth_key, created_at) VALUES ('conv_legacy', 'simplex://example/queue', 'queue_legacy', 'example.onion', ?1, 1);",
            rusqlite::params![vec![0x22u8; 32]],
        )
        .expect("legacy conv");
        conn.execute(
            "INSERT INTO messages (id, conversation_id, direction, body, timestamp, status, expires_at) VALUES ('msg_legacy', 'conv_legacy', 'inbound', ?1, 1, 'received', NULL);",
            rusqlite::params![b"legacy hello".to_vec()],
        )
        .expect("legacy msg");
    }
    // Sanity: the legacy file really is plaintext before migration.
    let raw_before = fs::read(&db_path).expect("read legacy file");
    assert_eq!(&raw_before[0..16], b"SQLite format 3\0");

    // First unlock after upgrade migrates; chats must survive.
    let app = SimpleSignalApp::init(db_str, pin).expect("migrated init");
    let convs = app.list_conversations().expect("list convs");
    assert!(convs.iter().any(|c| c.id == "conv_legacy"));
    let msgs = app.list_messages("conv_legacy").expect("list msgs");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].body_text, "legacy hello");
    let link1 = app.get_my_contact_link().expect("link after migration");
    drop(app);

    // Second unlock uses the new salt + encrypted file, same identity.
    let app2 = SimpleSignalApp::init(db_str, pin).expect("second init after migration");
    assert_eq!(link1, app2.get_my_contact_link().expect("link stable"));
    assert!(
        fs::read_to_string(format!("{}.salt", db_str)).is_ok(),
        "Salt file must exist after migration"
    );
    let raw_after = fs::read(&db_path).expect("read migrated file");
    assert_ne!(&raw_after[0..16], b"SQLite format 3\0");
}
