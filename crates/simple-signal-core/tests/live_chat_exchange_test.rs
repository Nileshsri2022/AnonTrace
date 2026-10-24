use simple_signal_core::api::SimpleSignalApp;
use tempfile::tempdir;

#[tokio::test]
async fn test_two_party_live_messaging_exchange_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let alice_db = dir.path().join("alice.db");
    let bob_db = dir.path().join("bob.db");

    // 1. Initialize Alice and Bob applications (simulating two phones)
    let alice_app =
        SimpleSignalApp::init(alice_db.to_str().unwrap(), "alice_pin_1111").expect("alice init");
    let bob_app =
        SimpleSignalApp::init(bob_db.to_str().unwrap(), "bob_pin_2222").expect("bob init");

    // 2. Both generate their standardized contact links (contains their respective receive queues)
    let alice_link_uri = alice_app.get_my_contact_link().expect("alice link");
    let bob_link_uri = bob_app.get_my_contact_link().expect("bob link");

    // 3. Bidirectional QR exchange: Bob imports Alice, Alice imports Bob
    let bob_conv = bob_app
        .import_contact_link(&alice_link_uri)
        .expect("bob imports alice");
    let alice_conv = alice_app
        .import_contact_link(&bob_link_uri)
        .expect("alice imports bob");

    // 4. Bob sends an encrypted message to Alice
    let outbound_msg = bob_app
        .send_chat_message(&bob_conv.id, "Hello Alice, post-quantum live message!")
        .expect("bob sends message");
    assert_eq!(outbound_msg.direction, "outbound");
    assert_eq!(outbound_msg.body_text, "Hello Alice, post-quantum live message!");

    // 5. Alice polls / receives incoming messages on her queue
    let alice_received = alice_app
        .receive_incoming_messages(&alice_conv.id)
        .expect("alice receives incoming");

    assert_eq!(alice_received.len(), 1, "Alice should receive Bob's message");
    assert_eq!(alice_received[0].direction, "inbound");
    assert_eq!(
        alice_received[0].body_text,
        "Hello Alice, post-quantum live message!"
    );

    // 6. Verify subsequent poll returns empty (queue was ACK'd and purged)
    let alice_empty = alice_app
        .receive_incoming_messages(&alice_conv.id)
        .expect("alice second poll");
    assert_eq!(alice_empty.len(), 0, "Queue should be empty after ACK");

    // 7. Alice replies to Bob
    let alice_reply = alice_app
        .send_chat_message(&alice_conv.id, "Hi Bob, decrypted cleanly with ML-KEM-1024!")
        .expect("alice replies");
    assert_eq!(alice_reply.direction, "outbound");

    // 8. Bob polls / receives Alice's reply
    let bob_received = bob_app
        .receive_incoming_messages(&bob_conv.id)
        .expect("bob receives reply");
    assert_eq!(bob_received.len(), 1);
    assert_eq!(bob_received[0].direction, "inbound");
    assert_eq!(
        bob_received[0].body_text,
        "Hi Bob, decrypted cleanly with ML-KEM-1024!"
    );
}

#[tokio::test]
async fn test_one_way_qr_scan_and_auto_handshake_messaging() {
    let dir = tempdir().expect("tempdir");
    let alice_db = dir.path().join("alice_single.db");
    let bob_db = dir.path().join("bob_single.db");

    let alice_app =
        SimpleSignalApp::init(alice_db.to_str().unwrap(), "alice_pin").expect("alice init");
    let bob_app =
        SimpleSignalApp::init(bob_db.to_str().unwrap(), "bob_pin").expect("bob init");

    // Alice shows her QR code
    let alice_link_uri = alice_app.get_my_contact_link().expect("alice link");

    // Only Bob scans Alice's QR code (Alice does NOT scan Bob!)
    let bob_conv = bob_app
        .import_contact_link(&alice_link_uri)
        .expect("bob imports alice");

    // Bob sends a message immediately
    bob_app
        .send_chat_message(&bob_conv.id, "Hey Alice, scanned your QR code!")
        .expect("bob sends message");

    // Alice polls her inbox without previously having imported Bob
    let alice_received = alice_app.sync_inbox().expect("alice sync inbox");
    assert_eq!(alice_received.len(), 1);
    assert_eq!(alice_received[0].body_text, "Hey Alice, scanned your QR code!");
    let alice_conv_id = &alice_received[0].conversation_id;

    // Alice replies to Bob using the automatically discovered conversation
    alice_app
        .send_chat_message(alice_conv_id, "Got it! No mutual QR scan required!")
        .expect("alice replies");

    // Bob polls and receives Alice's reply
    let bob_received = bob_app
        .receive_incoming_messages(&bob_conv.id)
        .expect("bob receives reply");
    assert_eq!(bob_received.len(), 1);
    assert_eq!(bob_received[0].body_text, "Got it! No mutual QR scan required!");
}

#[test]
fn test_two_party_raw_wire_payload_exchange_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let alice_db = dir.path().join("alice_wire.db");
    let bob_db = dir.path().join("bob_wire.db");

    let alice_app =
        SimpleSignalApp::init(alice_db.to_str().unwrap(), "alice_wire_pin").expect("alice init");
    let bob_app =
        SimpleSignalApp::init(bob_db.to_str().unwrap(), "bob_wire_pin").expect("bob init");

    // 1. Alice gets link & inbox queue
    let alice_link_uri = alice_app.get_my_contact_link().expect("alice link");
    let alice_queue = alice_app.my_inbox_queue_id();
    assert_eq!(alice_queue.len(), 64, "Blind queue id must be 64 hex chars");
    assert!(alice_queue.chars().all(|c| c.is_ascii_hexdigit()));

    // 2. Bob imports Alice's link via import_contact_link_with_handshake
    let (bob_conv, partner_queue, handshake_hex) = bob_app
        .import_contact_link_with_handshake(&alice_link_uri)
        .expect("bob import handshake");
    assert_eq!(partner_queue, alice_queue);
    assert!(!handshake_hex.is_empty());

    // 3. Alice receives handshake payload over the wire (no SmpRelayHub used)
    let handshake_bytes = hex::decode(&handshake_hex).expect("decode handshake");
    let maybe_msg = alice_app
        .process_incoming_payload(&handshake_bytes)
        .expect("alice process handshake");
    assert!(maybe_msg.is_none(), "Handshake produces no chat message");

    // 4. Bob sends a message: prepare_outbound_chat_message
    let (bob_msg, msg_partner_queue, payload_hex) = bob_app
        .prepare_outbound_chat_message(&bob_conv.id, "Real wire payload over HTTPS port 443!")
        .expect("bob prepare message");
    assert_eq!(msg_partner_queue, alice_queue);
    assert_eq!(bob_msg.body_text, "Real wire payload over HTTPS port 443!");

    // 5. Alice receives Bob's message payload over the wire
    let payload_bytes = hex::decode(&payload_hex).expect("decode payload");
    let decrypted = alice_app
        .process_incoming_payload(&payload_bytes)
        .expect("alice process payload")
        .expect("alice message decrypted");
    assert_eq!(decrypted.direction, "inbound");
    assert_eq!(decrypted.body_text, "Real wire payload over HTTPS port 443!");

    // 6. Alice replies: prepare_outbound_chat_message
    let (_alice_reply, bob_queue, reply_hex) = alice_app
        .prepare_outbound_chat_message(&decrypted.conversation_id, "Alice received message over wire!")
        .expect("alice prepare reply");
    assert_eq!(bob_queue, bob_app.my_inbox_queue_id());

    // 7. Bob receives Alice's reply payload over the wire
    let reply_bytes = hex::decode(&reply_hex).expect("decode reply");
    let bob_decrypted = bob_app
        .process_incoming_payload(&reply_bytes)
        .expect("bob process reply")
        .expect("bob message decrypted");
    assert_eq!(bob_decrypted.direction, "inbound");
    assert_eq!(bob_decrypted.body_text, "Alice received message over wire!");
}
