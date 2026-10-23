use simple_signal_core::storage::db::{derive_key, EncryptedDatabase, KeyDerivationParams, StorageError};
use tempfile::tempdir;

#[test]
fn test_database_open_with_correct_key_and_reject_wrong_key() {
    let dir = tempdir().expect("create temp dir");
    let db_path = dir.path().join("test_secure.db");

    let params = KeyDerivationParams {
        memory_kib: 8 * 1024, // fast for test
        iterations: 1,
        parallelism: 1,
        salt: [0x55; 16],
    };

    let correct_key = derive_key("master-password-123", &params)
        .expect("derive correct key");
    let wrong_key = derive_key("wrong-password-456", &params)
        .expect("derive wrong key");

    // 1. First open: creates encrypted database & schema
    {
        let db = EncryptedDatabase::open_or_create(&db_path, &correct_key)
            .expect("First open with correct key should succeed");
        
        // Verify WAL mode is active
        let journal_mode = db.get_journal_mode().expect("get journal mode");
        assert_eq!(journal_mode.to_uppercase(), "WAL");

        // Verify required tables exist
        let tables = db.list_tables().expect("list tables");
        for required in &[
            "identity_keys",
            "prekeys",
            "kyber_prekeys",
            "conversations",
            "sessions",
            "messages",
        ] {
            assert!(
                tables.contains(&required.to_string()),
                "Database schema missing required table: {}",
                required
            );
        }
    }

    // 2. Second open with WRONG key MUST fail with AuthenticationFailed
    {
        let result = EncryptedDatabase::open_or_create(&db_path, &wrong_key);
        assert!(result.is_err(), "Opening with wrong key must fail");
        match result {
            Err(StorageError::AuthenticationFailed) => {}
            other => panic!("Expected StorageError::AuthenticationFailed, got: {:?}", other),
        }
    }

    // 3. Third open with CORRECT key again MUST succeed
    {
        let db = EncryptedDatabase::open_or_create(&db_path, &correct_key)
            .expect("Re-opening with correct key should succeed");
        let tables = db.list_tables().expect("list tables on reopen");
        assert!(tables.contains(&"conversations".to_string()));
    }
}
