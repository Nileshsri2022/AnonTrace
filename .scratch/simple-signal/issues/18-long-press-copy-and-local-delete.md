# 18. Long-Press Message Actions (Copy & Local Delete)

| Property | Value |
| :--- | :--- |
| **Status** | Completed |
| **Type** | Feature / Security & UX |
| **Release Target** | v1.0.14 |
| **Blockers** | 17 (Timeline Grouping Adapter) |

---

## 1. Context & User Story
Users need standard chat interactions:
1. Long-pressing a message bubble to copy its plaintext content to the system clipboard.
2. Purging sensitive messages from local encrypted storage ("Delete for Me") without disturbing peer sessions or emitting outbound network telemetry.

---

## 2. Technical Scope

### 2.1 Storage Engine (`crates/simple-signal-core/src/storage/db.rs`)
Implement `delete_message(&self, message_id: &str) -> Result<bool, StorageError>`:
```sql
DELETE FROM messages WHERE id = ?1;
```

### 2.2 App API & C-ABI FFI (`crates/simple-signal-core/src/ffi.rs`)
Expose:
```rust
#[no_mangle]
pub unsafe extern "C" fn simple_signal_delete_message(message_id: *const c_char) -> c_int;
```
Returns `0` on success, error code on failure.

### 2.3 Dart FFI Bridge (`apps/simple_signal_mobile/lib/src/ffi/bridge.dart`)
Add `Future<bool> deleteMessage(String messageId);` to:
- `bridge.dart`
- `bridge_ffi.dart`
- `bridge_stub.dart`

### 2.4 ChatScreen Interaction (`apps/simple_signal_mobile/lib/src/screens/chat_screen.dart`)
- Wrap message bubbles with `InkWell(onLongPress: () => _showMessageActionSheet(context, bubble))`
- Action Sheet provides:
  - **Copy Text**: Copies to `Clipboard.setData` and displays a confirmation SnackBar.
  - **Delete for Me**: Prompts deletion confirmation dialog, calls `widget.onDeleteMessage?.call(msg.id)`, and removes the message from the local UI state list.

---

## 3. Verification Criteria
- [x] Rust core unit test verifies `delete_message` removes record from SQLite.
- [x] C-ABI FFI layer test verifies `simple_signal_delete_message`.
- [x] Dart widget test verifies long-press displays copy and delete options.
- [x] All Flutter tests (`flutter test`) and Rust tests (`cargo test --locked`) pass.
