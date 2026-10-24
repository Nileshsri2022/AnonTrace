# 10: SimpleX-Compatible HTTPS Port 443 Relay Queue Transport

**What to build:** An internet-facing HTTPS transport client (`RelayClient`) operating on TLS port 443 over ephemeral unidirectional queues, integrated with Rust C-ABI FFI wire helpers (`simple_signal_get_inbox_queue`, `simple_signal_process_payload`, `prepare_outbound_chat_message`) to enable real-time bidirectional message exchange across physical devices on mobile cellular networks.

**Blocked by:** 08: Privacy-Scrubbed In-App Diagnostic Logging & UI Viewer, 09: Compact Optical QR Code & One-Way Auto-Handshake Flow.

**Status:** closed

- [x] Builds `RelayClient` using `dart:io` `HttpClient` targeting TLS port 443 to bypass mobile carrier port filtering (Jio, Airtel, Vi).
- [x] Implements unidirectional `sendPayload` and ephemeral `pollQueue` methods with connection lifecycle isolation.
- [x] Exports `simple_signal_get_inbox_queue` from Rust FFI to generate deterministic inbox identifiers per device.
- [x] Exports `simple_signal_process_payload` to parse raw framed wire payloads (`0x00` handshake, `0x01` Double Ratchet ciphertext).
- [x] Wires Flutter `NativeSimpleSignalBridge` to automatically dispatch ciphertexts on send and poll inboxes every sync tick.
- [x] Verifies end-to-end payload roundtrip and multi-device exchange with unit and widget tests.
