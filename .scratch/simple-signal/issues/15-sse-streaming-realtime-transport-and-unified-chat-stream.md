# 15: Server-Sent Events (SSE) Streaming Transport & Unified Chat Stream

**What to build:** An instant real-time message delivery pipeline replacing aggressive 1.5s short-polling with persistent HTTP chunked streaming (SSE) on the inbox queue, coupled with an application-wide message event stream in Flutter that eliminates UI race conditions between `main.dart` background sync and active `ChatScreen` instances.

**Blocked by:** 14: SimpleX-Compatible HTTPS Port 443 Relay Queue Transport.

**Status:** closed

- [x] Implements persistent SSE stream listener (`listenToQueue`) in `RelayClient` using chunked NDJSON line decoding over a single Port 443 TLS connection.
- [x] Incorporates automatic exponential backoff reconnection on mobile network drops or 4G cell tower transitions.
- [x] Connects incoming streamed payloads directly to Rust FFI `simple_signal_process_payload` for instant Double Ratchet decryption.
- [x] Unifies message distribution through a centralized Dart `StreamController<MessageSummary>.broadcast()` in `SimpleSignalBridge`.
- [x] Updates `ChatScreen` to subscribe to the unified message stream and reload local messages, rendering incoming message bubbles immediately (< 50ms) without needing to back out and reopen the screen.
- [x] Maintains keep-alive connection pooling for outbound `sendPayload` dispatches, cutting outbound latency from ~1000ms to < 200ms.
- [x] Verifies streaming pipeline, reconnect behavior, and multi-listener delivery with automated unit and widget tests.
