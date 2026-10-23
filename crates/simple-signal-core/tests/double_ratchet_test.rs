use rand::rngs::OsRng;
use simple_signal_core::crypto::ratchet::DoubleRatchetSession;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

#[test]
fn test_double_ratchet_sequential_ping_pong() {
    let shared_root_key = [0x5Au8; 32];

    // Bob has an initial ratchet keypair (e.g. from his published signed prekey)
    let bob_dh_secret = StaticSecret::random_from_rng(&mut OsRng);
    let bob_dh_public = *XPublicKey::from(&bob_dh_secret).as_bytes();

    // Alice is the initiator (knows Bob's initial DH public key), Bob is the receiver
    let mut alice = DoubleRatchetSession::new_initiator(&shared_root_key, bob_dh_public);
    let mut bob = DoubleRatchetSession::new_receiver(&shared_root_key, bob_dh_secret);

    // 1. Alice -> Bob: Message 1
    let msg1_plain = b"Alice: Hello Bob!";
    let msg1_enc = alice.encrypt(msg1_plain).expect("Alice encrypt msg1");
    let msg1_dec = bob.decrypt(&msg1_enc).expect("Bob decrypt msg1");
    assert_eq!(msg1_dec, msg1_plain);

    // 2. Alice -> Bob: Message 2 (same DH ratchet, next symmetric chain step)
    let msg2_plain = b"Alice: Are you there?";
    let msg2_enc = alice.encrypt(msg2_plain).expect("Alice encrypt msg2");
    let msg2_dec = bob.decrypt(&msg2_enc).expect("Bob decrypt msg2");
    assert_eq!(msg2_dec, msg2_plain);

    // 3. Bob -> Alice: Reply Message 1 (Bob turns ratchet, advances DH)
    let reply1_plain = b"Bob: Yes Alice, loud and clear over Tor!";
    let reply1_enc = bob.encrypt(reply1_plain).expect("Bob encrypt reply1");
    let reply1_dec = alice.decrypt(&reply1_enc).expect("Alice decrypt reply1");
    assert_eq!(reply1_dec, reply1_plain);

    // 4. Alice -> Bob: Reply Message 2 (Alice turns ratchet again)
    let reply2_plain = b"Alice: Perfect forward secrecy verified.";
    let reply2_enc = alice.encrypt(reply2_plain).expect("Alice encrypt reply2");
    let reply2_dec = bob.decrypt(&reply2_enc).expect("Bob decrypt reply2");
    assert_eq!(reply2_dec, reply2_plain);
}

#[test]
fn test_double_ratchet_out_of_order_message_delivery() {
    let shared_root_key = [0x7Eu8; 32];

    let bob_dh_secret = StaticSecret::random_from_rng(&mut OsRng);
    let bob_dh_public = *XPublicKey::from(&bob_dh_secret).as_bytes();

    let mut alice = DoubleRatchetSession::new_initiator(&shared_root_key, bob_dh_public);
    let mut bob = DoubleRatchetSession::new_receiver(&shared_root_key, bob_dh_secret);

    // Alice sends three messages in succession
    let msg1 = alice.encrypt(b"Message 1").expect("encrypt 1");
    let msg2 = alice.encrypt(b"Message 2").expect("encrypt 2");
    let msg3 = alice.encrypt(b"Message 3").expect("encrypt 3");

    // Out-of-order delivery to Bob: msg2 arrives first!
    let dec2 = bob.decrypt(&msg2).expect("Bob should decrypt msg2 out of order");
    assert_eq!(dec2, b"Message 2");

    // Then msg3 arrives
    let dec3 = bob.decrypt(&msg3).expect("Bob should decrypt msg3 out of order");
    assert_eq!(dec3, b"Message 3");

    // Finally msg1 arrives (delayed/reordered packet)
    let dec1 = bob.decrypt(&msg1).expect("Bob should decrypt delayed msg1 from skipped keys");
    assert_eq!(dec1, b"Message 1");
}
