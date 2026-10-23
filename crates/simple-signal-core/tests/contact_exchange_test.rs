use tempfile::tempdir;

use simple_signal_core::contact::handshake::{
    accept_contact_handshake, initiate_contact_exchange, ContactHandshakeMessage, QueueCoordinates,
};
use simple_signal_core::contact::link::ContactLink;
use simple_signal_core::contact::storage::persist_contact_conversation;
use simple_signal_core::crypto::pqxdh::{generate_identity_keypair, generate_prekey_bundle};
use simple_signal_core::crypto::ratchet::DoubleRatchetSession;
use simple_signal_core::storage::db::{derive_key, EncryptedDatabase, KeyDerivationParams};

#[test]
fn test_end_to_end_contact_exchange_and_storage_binding() {
    // 1. Alice creates her identity, prekey bundle, and ContactLink
    let alice_identity = generate_identity_keypair();
    let (alice_bundle, alice_secrets) = generate_prekey_bundle(&alice_identity, 1, 10);

    let alice_link = ContactLink {
        relay_onion: "aliceonionrelay7239478234y23847928374982374982374982374982.onion".to_string(),
        receive_queue_id: "q_alice_inbox_01".to_string(),
        sender_auth_key: vec![0xAA; 32],
        prekey_bundle: alice_bundle,
    };

    // Alice exports link to URI (e.g. into QR code)
    let uri = alice_link.to_uri().expect("Alice to_uri");

    // 2. Bob scans Alice's QR code / URI
    let bob_parsed_link = ContactLink::from_uri(&uri).expect("Bob from_uri");

    let bob_identity = generate_identity_keypair();
    let bob_return_queue = QueueCoordinates {
        relay_onion: "bobonionrelay1111111111111111111111111111111111111111111111.onion".to_string(),
        queue_id: "q_bob_inbox_01".to_string(),
        sender_auth_key: vec![0xBB; 32],
    };

    // Bob accepts link, produces handshake message and his ratchet session
    let (handshake_msg, mut bob_session) = initiate_contact_exchange(
        &bob_parsed_link,
        &bob_identity,
        bob_return_queue.clone(),
    )
    .expect("Bob initiate exchange");

    // Wire serialization roundtrip
    let handshake_bytes = handshake_msg.to_bytes().expect("serialize handshake");
    let received_handshake =
        ContactHandshakeMessage::from_bytes(&handshake_bytes).expect("deserialize handshake");

    // 3. Alice receives handshake on her receive queue and initializes her ratchet session
    let (mut alice_session, return_coords) =
        accept_contact_handshake(&received_handshake, &alice_identity, &alice_secrets)
            .expect("Alice accept handshake");

    assert_eq!(return_coords, bob_return_queue);

    // 4. Test bidirectional Double Ratchet messaging between Alice and Bob
    let bob_first_msg = b"Hello Alice, this is Bob established via Simple+Signal!";
    let enc1 = bob_session.encrypt(bob_first_msg).expect("Bob encrypts");
    let dec1 = alice_session.decrypt(&enc1).expect("Alice decrypts");
    assert_eq!(dec1, bob_first_msg);

    let alice_reply_msg = b"Hello Bob! Verified PQXDH + Ratchet over Tor.";
    let enc2 = alice_session.encrypt(alice_reply_msg).expect("Alice encrypts");
    let dec2 = bob_session.decrypt(&enc2).expect("Bob decrypts");
    assert_eq!(dec2, alice_reply_msg);

    // 5. Seam 3: Storage Binding
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("bob_store.db");

    let params = KeyDerivationParams {
        memory_kib: 8192,
        iterations: 1,
        parallelism: 1,
        salt: [0x55; 16],
    };
    let key = derive_key("master-passphrase-test", &params).expect("derive key");
    let db = EncryptedDatabase::open_or_create(&db_path, &key).expect("open db");

    let conversation_id = "conv_alice_and_bob_01";
    let bob_session_bytes = bob_session.to_bytes().expect("bob session to_bytes");

    persist_contact_conversation(
        &db,
        conversation_id,
        &QueueCoordinates {
            relay_onion: alice_link.relay_onion.clone(),
            queue_id: alice_link.receive_queue_id.clone(),
            sender_auth_key: alice_link.sender_auth_key.clone(),
        },
        &bob_return_queue.queue_id,
        &bob_session_bytes,
    )
    .expect("persist bob contact");

    let conv = db.get_conversation(conversation_id).expect("get conv").unwrap();
    assert_eq!(conv.receive_queue_id, "q_bob_inbox_01");
    assert_eq!(conv.relay_onion_address, alice_link.relay_onion);
    assert_eq!(db.count_sessions(conversation_id).expect("count sessions"), 1);

    // 6. Reload Bob's session from storage and ensure ratchet continues seamlessly
    let reloaded_bob_session =
        DoubleRatchetSession::from_bytes(&bob_session_bytes).expect("reload bob session");
    let mut reloaded_bob_session = reloaded_bob_session;

    let alice_msg3 = b"Alice: Can you read this after reload?";
    let enc3 = alice_session.encrypt(alice_msg3).expect("Alice encrypts msg3");
    let dec3 = reloaded_bob_session.decrypt(&enc3).expect("Reloaded Bob decrypts msg3");
    assert_eq!(dec3, alice_msg3);
}
