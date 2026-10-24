# 21. E2EE Delivery ACKs and WhatsApp Message Status Ticks

| Property | Value |
| :--- | :--- |
| **Status** | Completed |
| **Type** | Feature / Protocol & UI |
| **Release Target** | v1.0.14 |
| **Blockers** | 20 (Swipe-to-Reply) |

---

## 1. Context & User Story
Users need delivery confirmation:
- Single Grey Tick (`✓`): Sent to relay outbox queue.
- Double Grey Tick (`✓✓`): Delivered & decrypted on recipient phone.
- Double Blue Tick (`✓✓`): Viewed by recipient in active ChatScreen.

---

## 2. Technical Scope

### 2.1 Storage Status Updates (`crates/simple-signal-core/src/storage/db.rs`)
Implement:
```rust
pub fn update_message_status(&self, message_id: &str, status: &str) -> Result<bool, StorageError>;
```
Updates `status` column in `messages` table.

### 2.2 Wire Protocol Frame 0x02 (`crates/simple-signal-core/src/api.rs`)
Handle `0x02` in `process_payload`:
- Decrypts ACK payload with active Double Ratchet session.
- Parses `{"ack_msg_id": "...", "status": "delivered" | "read"}`.
- Updates database record status.

### 2.3 C-ABI Export & Dart Bridge
Expose:
- `simple_signal_update_message_status(message_id, status)` in Rust FFI.
- `Future<bool> updateMessageStatus({required String messageId, required String status})` in Dart bridge.

### 2.4 ChatScreen Visual Ticks (`apps/simple_signal_mobile/lib/src/screens/chat_screen.dart`)
Render tick icons based on `msg.status`:
- `'sent'`: `Icon(Icons.done, size: 13, color: Colors.white70)`
- `'delivered'`: `Icon(Icons.done_all, size: 14, color: Colors.white70)`
- `'read'`: `Icon(Icons.done_all, size: 14, color: Color(0xFF34B7F1))`

---

## 3. Verification Criteria
- [x] Rust core unit test verifies `update_message_status`.
- [x] Wire framing `0x02` ACK processing updates status in test.
- [x] Dart Bridge exports `updateMessageStatus`.
- [x] ChatScreen renders single grey tick, double grey ticks, and double blue ticks according to status.
- [x] All 33 Rust tests and 24 Flutter tests pass.
- [ ] Widget test verifies rendering of single tick, double grey tick, and double blue tick.
- [ ] All Flutter tests and Rust tests pass.
