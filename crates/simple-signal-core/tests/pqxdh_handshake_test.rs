use simple_signal_core::crypto::pqxdh::{
    generate_identity_keypair, generate_prekey_bundle, initiate_pqxdh, process_pqxdh_initiation,
};

#[test]
fn test_pqxdh_handshake_derives_matching_post_quantum_root_keys() {
    // 1. Alice creates identity and publishes PreKeyBundle
    let alice_identity = generate_identity_keypair();
    let (alice_bundle, alice_secrets) = generate_prekey_bundle(&alice_identity, 1, 100);

    // Verify signature on signed prekey
    assert!(alice_bundle.verify_signature(), "Signed prekey signature must be valid");

    // 2. Bob initiates PQXDH handshake with Alice's PreKeyBundle
    let bob_identity = generate_identity_keypair();
    let (pqxdh_message, bob_shared_secret) = initiate_pqxdh(&bob_identity, &alice_bundle)
        .expect("Bob PQXDH initiation should succeed");

    assert_eq!(bob_shared_secret.len(), 32);

    // 3. Alice receives and processes Bob's PQXDH initiation
    let alice_shared_secret = process_pqxdh_initiation(
        &alice_identity,
        &alice_secrets,
        &pqxdh_message,
    )
    .expect("Alice PQXDH processing should succeed");

    assert_eq!(alice_shared_secret.len(), 32);

    // Both Alice and Bob derive the exact same 32-byte post-quantum root key!
    assert_eq!(
        alice_shared_secret, bob_shared_secret,
        "Alice and Bob post-quantum root keys must be identical"
    );
}

#[test]
fn test_pqxdh_fails_with_invalid_signature() {
    let alice_identity = generate_identity_keypair();
    let (mut alice_bundle, _) = generate_prekey_bundle(&alice_identity, 1, 100);

    // Corrupt the signature
    alice_bundle.signature[0] ^= 0xFF;
    assert!(!alice_bundle.verify_signature(), "Tampered signature must fail verification");

    let bob_identity = generate_identity_keypair();
    let result = initiate_pqxdh(&bob_identity, &alice_bundle);
    assert!(result.is_err(), "Initiating handshake with invalid signature must fail");
}
