use simple_signal_core::net::tor::{
    format_relay_post, parse_relay_response, StealthState, TorClientManager,
};

#[test]
fn stealth_status_is_not_started_before_bootstrap() {
    let manager = TorClientManager::default_client();
    let status = manager.stealth_status();
    assert_eq!(status.state, StealthState::NotStarted);
    assert_eq!(status.frac, 0.0);
    assert!(!status.ready_for_traffic);
    assert_eq!(status.onion_address, None);
}

#[test]
fn stealth_bootstrap_rejects_invalid_onion_fast() {
    let manager = TorClientManager::default_client();
    let dir = tempfile::TempDir::new().unwrap();
    let err = manager
        .start_stealth_bootstrap(dir.path(), Some("not-an-onion".to_string()))
        .unwrap_err();
    assert!(err.to_string().contains("onion"));
    // No bootstrap was started.
    assert_eq!(manager.stealth_status().state, StealthState::NotStarted);
}

#[test]
fn isolation_tokens_are_per_queue_and_deterministic() {
    let manager = TorClientManager::default_client();
    let a = manager.isolation_token_for_queue("queue-a");
    let b = manager.isolation_token_for_queue("queue-b");
    assert_ne!(a, b);
    assert_eq!(a, manager.isolation_token_for_queue("queue-a"));
}

#[test]
fn relay_post_wire_shape_and_response_parsing() {
    let req = format_relay_post("blind123", "example.onion", "aabb", "deadbeef");
    assert!(req.starts_with("POST /q/blind123 HTTP/1.1\r\n"));
    assert!(req.contains("Host: example.onion\r\n"));
    assert!(req.contains("X-Sender-Auth: aabb\r\n"));
    assert!(req.contains("Content-Length: 8\r\n"));
    assert!(req.ends_with("deadbeef"));

    let ok = parse_relay_response("HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\n{\"id\":\"m1\"}").unwrap();
    assert_eq!(ok, "{\"id\":\"m1\"}");
    assert!(parse_relay_response("HTTP/1.1 403 Forbidden\r\n\r\n{}").is_err());
}
