# 07: Cross-Platform Flutter UI & FFI Bridge Integration

**What to build:** A cross-platform Flutter application binding directly to `simple-signal-core` via `flutter_rust_bridge`, featuring a PIN/biometric unlock screen, interactive chat conversations, QR code connection generator/scanner, file attachment support, and an Android Foreground Service (`FOREGROUND_SERVICE_REMOTE_MESSAGING`) to maintain reliable background notifications.

**Blocked by:** 05: Contact Link & QR Exchange Module, 06: Decoupled Push Notification Blinding Gateway.

**Status:** closed

- [x] Generates typed FFI bindings between the Rust core and Dart using `flutter_rust_bridge`.
- [x] Renders lock screen with Master PIN and Biometric authentication before decrypting local storage.
- [x] Provides real-time chat interface showing inbound and outbound messages with delivery statuses.
- [x] Includes integrated camera QR code scanner and Contact Link generator.
- [x] Configures Android `FOREGROUND_SERVICE_REMOTE_MESSAGING` and `WAKE_LOCK` for continuous background delivery.
