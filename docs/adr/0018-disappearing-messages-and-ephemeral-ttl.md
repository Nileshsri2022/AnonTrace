# ADR-0018: Ephemeral Message TTL and Local Ephemeral Purge Cycle

## Status
Accepted

## Context
Privacy-focused users require automated ephemeral message deletion (Disappearing Messages) to limit data retention on physical devices and prevent forensic extraction in the event of device seizure or loss.
Traditional centralized platforms (e.g. WhatsApp, Telegram) manage message expiration through central database timers or sync servers. In AnonTrace's architecture:
1. There is no central server, user account, or database.
2. Relays are zero-knowledge, dumb packet brokers over unidirectional queues (SimpleX model) that discard ciphertexts immediately once dequeued by the recipient.
3. End-to-end encryption utilizes ML-KEM-1024, X25519, and Double Ratchet (AES-256-GCM).
4. Ephemeral deletion must strictly happen client-side within the local encrypted SQLite storage without relying on server timers or generating unencrypted telemetry.

## Decision
1. **Schema Extension**:
   - Add `expires_at INTEGER DEFAULT NULL` column to `messages` table in `crates/simple-signal-core/src/storage/db.rs`.
   - Add `disappearing_timer_seconds INTEGER DEFAULT 0` column to `conversations` table.
   - When a conversation has `disappearing_timer_seconds > 0`, any newly sent or received message calculates `expires_at = timestamp + disappearing_timer_seconds`.
2. **Atomic Purge Sweep (`Ephemeral Purge Cycle`)**:
   - Provide `EncryptedDatabase::purge_expired_messages(&self, current_timestamp: i64) -> Result<usize, StorageError>`.
   - Executes `DELETE FROM messages WHERE expires_at IS NOT NULL AND expires_at <= ?1;`.
   - All deleted rows are unlinked from WAL/database pages in encrypted storage.
3. **C-ABI FFI and Dart Bridge**:
   - Export `simple_signal_purge_expired_messages()` and `simple_signal_set_disappearing_timer(conv_id, seconds)`.
   - Dart bridge exposes:
     - `Future<int> purgeExpiredMessages();`
     - `Future<bool> setDisappearingTimer({required String conversationId, required int durationSeconds});`
4. **ChatScreen UI**:
   - Conversation options menu provides selectable TTL durations: Off (0s), 5 Minutes (300s), 1 Hour (3600s), 24 Hours (86400s), 7 Days (604800s).
   - An indicator badge or subtitle indicates active ephemeral timer (e.g. "Disappearing: 24h").
   - Purge cycle runs on screen initialization and periodic interval, dynamically removing expired message bubbles.

## Consequences
- **Positive**:
  - Zero metadata leakage to transport relays; relays never know whether messages are ephemeral or standard.
  - Zero persistent trace after expiration window passes.
  - Full cryptographic forward secrecy: expired messages are unrecoverable even if the device key is compromised later.
- **Negative**:
  - Clock skew between peers might result in slightly different expiration points if using sender timestamp; mitigated by computing TTL relative to local arrival/creation time.
