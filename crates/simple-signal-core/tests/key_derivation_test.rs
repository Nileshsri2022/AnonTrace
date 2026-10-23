use simple_signal_core::storage::db::{derive_key, KeyDerivationParams, StorageError};

#[test]
fn test_derive_key_produces_32_byte_aes_key() {
    let params = KeyDerivationParams {
        memory_kib: 64 * 1024, // 64 MB as per ADR 0004
        iterations: 3,
        parallelism: 4,
        salt: [0x42; 16],
    };

    let key1 = derive_key("correct-horse-battery-staple", &params)
        .expect("Key derivation should succeed with valid parameters");
    assert_eq!(key1.len(), 32);

    // Deterministic: same inputs produce exact same key
    let key2 = derive_key("correct-horse-battery-staple", &params)
        .expect("Key derivation should be deterministic");
    assert_eq!(key1, key2);

    // Distinct passphrase produces distinct key
    let key_diff_pass = derive_key("different-passphrase", &params)
        .expect("Different passphrase derivation should succeed");
    assert_ne!(key1, key_diff_pass);

    // Distinct salt produces distinct key
    let mut params_diff_salt = params.clone();
    params_diff_salt.salt = [0x99; 16];
    let key_diff_salt = derive_key("correct-horse-battery-staple", &params_diff_salt)
        .expect("Different salt derivation should succeed");
    assert_ne!(key1, key_diff_salt);
}

#[test]
fn test_derive_key_rejects_empty_or_too_small_memory() {
    let invalid_params = KeyDerivationParams {
        memory_kib: 1, // too small memory cost
        iterations: 1,
        parallelism: 1,
        salt: [0x42; 16],
    };

    let result = derive_key("any-passphrase", &invalid_params);
    assert!(result.is_err());
    match result {
        Err(StorageError::InvalidKeyDerivationParams(_)) => {}
        _ => panic!("Expected StorageError::InvalidKeyDerivationParams"),
    }
}
