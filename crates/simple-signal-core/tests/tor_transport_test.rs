use simple_signal_core::net::tor::{
    validate_onion_address, DormancyMode, TorClientManager, TorError, TorTransportConfig,
};
use tempfile::tempdir;

#[tokio::test]
async fn test_tor_client_manager_initialization_and_stream_isolation() {
    let dir = tempdir().expect("temp dir");
    let tor_data_dir = dir.path().join("tor_data");

    let config = TorTransportConfig {
        data_directory: tor_data_dir,
        socks_port: None, // strictly in-process, zero open local listening ports
        strict_tor_only: true, // zero clearnet fallback
    };

    let manager = TorClientManager::new(config).expect("create TorClientManager");

    // Verify stream isolation tokens:
    // Different pairwise queues MUST produce distinct circuit isolation tokens (isolate-by-auth)
    let token_queue1 = manager.isolation_token_for_queue("conv-alice-bob");
    let token_queue2 = manager.isolation_token_for_queue("conv-alice-charlie");
    let token_queue1_repeat = manager.isolation_token_for_queue("conv-alice-bob");

    assert_eq!(
        token_queue1, token_queue1_repeat,
        "Same queue must produce deterministic isolation token"
    );
    assert_ne!(
        token_queue1, token_queue2,
        "Different queues MUST produce cryptographically distinct isolation tokens"
    );
}

#[tokio::test]
async fn test_tor_dormancy_lifecycle_transitions() {
    let dir = tempdir().expect("temp dir");
    let config = TorTransportConfig {
        data_directory: dir.path().join("tor_data"),
        socks_port: None,
        strict_tor_only: true,
    };

    let manager = TorClientManager::new(config).expect("create manager");

    // Initially active
    assert_eq!(manager.dormancy_state(), DormancyMode::Active);

    // App goes to background -> enter soft dormant mode
    manager.set_dormancy(DormancyMode::Soft).expect("enter soft dormant");
    assert_eq!(manager.dormancy_state(), DormancyMode::Soft);

    // Incoming blinded push -> fast wakeup
    manager.wake_up().expect("wakeup");
    assert_eq!(manager.dormancy_state(), DormancyMode::Active);
}

#[test]
fn test_onion_address_syntax_validation() {
    // Valid Tor v3 onion address (56 characters base32 + .onion)
    let valid_v3 = "vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion";
    assert!(validate_onion_address(valid_v3).is_ok());

    // Invalid: Clearnet host (must be blocked by strict Tor policy!)
    let clearnet = "example.com";
    let err = validate_onion_address(clearnet).expect_err("clearnet host must be rejected");
    match err {
        TorError::InvalidOnionAddress(msg) => {
            assert!(msg.contains("must end with .onion"));
        }
        _ => panic!("Expected TorError::InvalidOnionAddress"),
    }

    // Invalid: Clearnet IP address
    let ip = "192.168.1.1";
    assert!(validate_onion_address(ip).is_err());

    // Invalid: Legacy Tor v2 onion (16 chars, discontinued)
    let v2 = "expyuzz5wqqfdgah.onion";
    assert!(validate_onion_address(v2).is_err());
}
