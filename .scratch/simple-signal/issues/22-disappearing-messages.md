# 22. Ephemeral Message TTL and Disappearing Messages

| Property | Value |
| :--- | :--- |
| **Status** | Completed |
| **Type** | Feature / Storage & UI |
| **Release Target** | v1.0.14 |
| **Blockers** | 21 (Message Ticks) |

---

## 1. Context & User Story
Users communicate sensitive, high-risk intelligence that must not persist indefinitely on local device storage. Enabling Disappearing Messages ensures that message records and cryptographic plaintexts self-destruct after a designated duration (e.g., 5 min, 1 hour, 24 hours).

---

## 2. Technical Scope

### 2.1 Storage Layer (`crates/simple-signal-core/src/storage/db.rs`)
1. Extend `messages` table with `expires_at INTEGER DEFAULT NULL`.
2. Extend `conversations` table with `disappearing_timer_seconds INTEGER DEFAULT 0`.
3. Add methods:
   - `set_conversation_disappearing_timer(&self, conv_id: &str, seconds: i64) -> Result<(), StorageError>`
   - `get_conversation_disappearing_timer(&self, conv_id: &str) -> Result<i64, StorageError>`
   - `purge_expired_messages(&self, current_timestamp: i64) -> Result<usize, StorageError>`
   - `list_messages` automatically filters out expired messages.

### 2.2 Core API & FFI (`crates/simple-signal-core/src/api.rs`, `src/ffi.rs`)
1. In `AppApi`:
   - `set_disappearing_timer(conv_id: &str, seconds: i64) -> Result<(), FfiError>`
   - `purge_expired_messages() -> Result<usize, FfiError>`
   - When storing outbound/inbound messages, populate `expires_at` if conversation has active timer > 0.
2. In FFI:
   - `simple_signal_set_disappearing_timer(conv_id: *const c_char, seconds: i64) -> *mut c_char`
   - `simple_signal_purge_expired_messages() -> *mut c_char`

### 2.3 Dart Bridge (`apps/simple_signal_mobile/lib/src/ffi/bridge.dart`, `bridge_ffi.dart`, `bridge_stub.dart`)
1. Add to `SimpleSignalBridge`:
   - `Future<bool> setDisappearingTimer({required String conversationId, required int durationSeconds});`
   - `Future<int> purgeExpiredMessages();`
2. Implement native C-ABI bindings and fallback simulations.

### 2.4 ChatScreen UI (`apps/simple_signal_mobile/lib/src/screens/chat_screen.dart`)
1. AppBar popup action to configure disappearing message duration.
2. Visual badge / header indicator showing active timer duration.
3. Message bubbles render `msg_timer_icon` for ephemeral messages.
4. Periodic purge check and UI update removing expired messages from viewport.

---

## 3. Verification Criteria
- [x] Unit test in `crates/simple-signal-core` verifying `expires_at` storage, retrieval, and `purge_expired_messages`.
- [x] Unit test verifying `list_messages` automatically purges expired messages.
- [x] Flutter widget test verifying disappearing timer selection menu, `1h`/`1d` indicators, and message bubble timer icons.
- [x] Full test suites (`cargo test --locked`, `flutter test`) passing cleanly.
