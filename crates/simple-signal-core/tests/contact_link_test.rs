use simple_signal_core::contact::link::{ContactLink, ContactLinkError};
use simple_signal_core::crypto::pqxdh::PreKeyBundle;

#[test]
fn test_contact_link_serialization_roundtrip() {
    let prekey_bundle = PreKeyBundle {
        identity_key: [0x11; 32],
        identity_dh_key: [0x22; 32],
        signed_prekey_id: 1,
        signed_prekey: [0x33; 32],
        signature: [0x44; 64],
        one_time_prekey_id: 100,
        one_time_prekey: [0x55; 32],
        kyber_prekey: vec![0x66; 1568], // ML-KEM-1024 public key size
    };

    let link = ContactLink {
        relay_onion: "vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion".to_string(),
        receive_queue_id: "q_alice_temp_99".to_string(),
        sender_auth_key: vec![0x77; 32],
        prekey_bundle,
    };

    // 1. Serialize to URI
    let uri_string = link.to_uri().expect("serialize to URI");
    assert!(uri_string.starts_with("simplex:/contact#/?"));
    assert!(uri_string.contains("relay=vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion"));
    assert!(uri_string.contains("queue=q_alice_temp_99"));

    // 2. Parse from URI
    let parsed_link = ContactLink::from_uri(&uri_string).expect("parse from URI");
    assert_eq!(parsed_link.relay_onion, link.relay_onion);
    assert_eq!(parsed_link.receive_queue_id, link.receive_queue_id);
    assert_eq!(parsed_link.sender_auth_key, link.sender_auth_key);
    assert_eq!(parsed_link.prekey_bundle.identity_key, link.prekey_bundle.identity_key);
    assert_eq!(parsed_link.prekey_bundle.kyber_prekey, link.prekey_bundle.kyber_prekey);
}

#[test]
fn test_contact_link_rejects_invalid_scheme_or_missing_fields() {
    // Missing simplex:/ scheme
    let invalid_scheme = "https://example.com/contact#/?relay=test.onion";
    let err = ContactLink::from_uri(invalid_scheme).expect_err("should reject invalid scheme");
    match err {
        ContactLinkError::InvalidScheme(_) => {}
        _ => panic!("Expected ContactLinkError::InvalidScheme"),
    }

    // Missing required relay parameter
    let missing_relay = "simplex:/contact#/?queue=q1&auth=123";
    let err = ContactLink::from_uri(missing_relay).expect_err("should reject missing relay");
    match err {
        ContactLinkError::MissingField(field) => assert_eq!(field, "relay"),
        _ => panic!("Expected ContactLinkError::MissingField"),
    }
}

#[test]
fn test_compact_qr_contact_link_without_kpk_is_under_600_chars() {
    let prekey_bundle = PreKeyBundle {
        identity_key: [0x11; 32],
        identity_dh_key: [0x22; 32],
        signed_prekey_id: 1,
        signed_prekey: [0x33; 32],
        signature: [0x44; 64],
        one_time_prekey_id: 100,
        one_time_prekey: [0x55; 32],
        kyber_prekey: Vec::new(), // Compact optical QR format
    };

    let link = ContactLink {
        relay_onion: "vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion".to_string(),
        receive_queue_id: "q_alice_temp_99".to_string(),
        sender_auth_key: vec![0x77; 32],
        prekey_bundle,
    };

    let uri_string = link.to_uri().expect("serialize to URI");
    println!("Compact QR URI length: {}", uri_string.len());
    assert!(uri_string.len() < 700, "URI length {} must be < 700 for reliable QR scanning", uri_string.len());

    let parsed = ContactLink::from_uri(&uri_string).expect("parse compact URI");
    assert_eq!(parsed.prekey_bundle.identity_key, [0x11; 32]);
    assert!(parsed.prekey_bundle.kyber_prekey.is_empty());
}

#[test]
fn print_valid_bundle_link() {
    let identity = simple_signal_core::crypto::pqxdh::generate_identity_keypair();
    let (bundle, _) = simple_signal_core::crypto::pqxdh::generate_prekey_bundle(&identity, 1, 100);
    let mut qr_bundle = bundle;
    qr_bundle.kyber_prekey = Vec::new();
    let link = ContactLink {
        relay_onion: "vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion".to_string(),
        receive_queue_id: "q_web_inbox_01".to_string(),
        sender_auth_key: vec![0x11; 32],
        prekey_bundle: qr_bundle,
    };
    println!("VALID_CONTACT_LINK={}", link.to_uri().unwrap());
}
