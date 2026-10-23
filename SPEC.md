# Functional and Architectural Specification: Simple+Signal

## Problem Statement

Modern private messengers force users to choose between two conflicting compromises:
1. **Convenience with Metadata Leakage (Signal / WhatsApp):** Even with state-of-the-art end-to-end encryption (Signal Protocol), centralized servers require permanent phone numbers or usernames. Central servers log who speaks to whom, communication timestamps, and IP addresses, while notification triggers sent to Apple (APNs) and Google (FCM) leak communication graph metadata to surveillance capitalism and state actors.
2. **Anonymity with Usability & Performance Penalties (SimpleX / Tor):** Pure anonymous routing without global identities requires managing ephemeral queues and network proxies. Users often face sluggish connection setups, complex daemon configurations (such as running external Orbot/Tor daemons that expose open local ports), and high mobile battery drain.

Users need an application that provides the **cryptographic speed, post-quantum security, and seamless UI of Signal**, combined with the **zero-identifier, metadata-eliminating queue routing of SimpleX over embedded Tor (Arti)**, operating seamlessly on Android and Desktop.

---

## Solution

**Simple+Signal** is a privacy-first, cross-platform messenger architected across four decoupled layers:
* **UI Layer:** A single cross-platform Flutter application providing responsive, modern messaging interactions across Android, Windows, and Linux.
* **Core & Bridge Layer:** A native Rust crate (`simple-signal-core`) bound to Flutter via `flutter_rust_bridge`, orchestrating asynchronous event loops and memory-safe cryptography.
* **Cryptography Layer:** Official Signal Protocol implementation via `libsignal` (Double Ratchet, PQXDH with Kyber-1024, AES-256-GCM, HMAC-SHA256).
* **Identity & Routing Layer:** SimpleX unidirectional Pairwise Queues ($Q_1, Q_2$) with zero global accounts, phone numbers, or central registries.
* **Network Layer:** In-process embedded Tor client (**Arti**) connecting exclusively to Tor v3 `.onion` relay endpoints with zero clearnet IP leakage.
* **Storage Layer:** SQLCipher local encrypted SQLite database derived via Argon2id from a user PIN/passphrase with biometric unlock.
* **Notifications:** Zero-knowledge decoupled push notification routing using ephemeral blinded tokens to wake up mobile devices without revealing participants or message contents.

---

## User Stories

1. As an anonymous user, I want to launch the application without providing a phone number, email, or username, so that my real-world identity is never tied to my communications.
2. As a user, I want to generate a Contact Link as a scannable QR Code or clipboard URI, so that I can establish a private connection with a partner in person or over an existing channel.
3. As a user, I want to scan a partner's Contact Link QR code using my device camera, so that an automated bidirectional cryptographic handshake is completed instantly.
4. As a user, I want my messages to be encrypted using Signal's Post-Quantum Double Ratchet (PQXDH), so that future quantum computers cannot decrypt captured traffic (Forward Secrecy and Post-Compromise Security).
5. As an anonymity-conscious user, I want all network traffic to route through embedded Tor (Arti) to Tor v3 `.onion` relays, so that my ISP and relay operators never see my public IP address.
6. As a user, I want the application to enforce strict Tor-only transport with no clearnet fallback, so that my IP address is never accidentally leaked during network disruptions.
7. As a mobile user, I want to receive push notifications when my phone is locked, so that I know when a new message has arrived without draining my battery running continuous Tor circuits.
8. As a privacy-conscious user, I want push notifications sent through Apple (APNs) or Google (FCM) to contain zero sender identity, recipient identity, or message text, so that platform operators cannot build a social graph of my contacts.
9. As a mobile user, I want Tor circuits to enter dormant sleep mode while the app is in the background, so that my battery life remains comparable to standard messaging apps.
10. As a user, I want to send text messages, images, voice notes, and file attachments up to 10MB, so that I can communicate richly without compromising privacy.
11. As a user, I want all local messages, cryptographic ratchets, and contact queues to be stored inside an encrypted SQLCipher database, so that my data is protected against physical extraction if my device is lost or seized.
12. As a security-conscious user, I want to protect my local database with an optional Master PIN/Passphrase using Argon2id, so that my data remains secure even on rooted or compromised hardware.
13. As a daily user, I want to unlock the application using device biometrics (fingerprint/face recognition), so that I do not need to type my master PIN on every launch.
14. As a user, I want to delete a contact and have the application cascade-delete all associated Pairwise Queues, Double Ratchet sessions, and chat history, so that zero forensic remnants remain on disk.
15. As a user, I want to export an encrypted backup file of my profile and import it onto a new device, so that I can migrate my chats and contacts securely.
16. As a user, I want every network packet to be padded to fixed 16,384-byte blocks, so that passive network observers cannot infer message types or lengths through traffic analysis.

---

## Implementation Decisions

### 1. Modular Architecture
* **`simple-signal-core` (Rust Crate):**
  * `net::tor`: Embedded `arti-client` management, circuit isolation (`isolate-by-auth`), and dormancy controllers (`set_dormant`).
  * `net::smp`: Async WebSocket client implementing SimpleX Messaging Protocol framing (`NEW`, `SEND`, `SUB`, `ACK`, `DEL`, `PING`) over Arti Tor streams.
  * `crypto::ratchet`: Wrapper around `libsignal` managing `Pqxdh` handshakes, `SessionRecord` persistence, and 16KB transport block padding.
  * `storage::db`: SQLCipher database interface managing migrations, key derivation (`argon2id`), and cascading transactional state.
  * `bridge::api`: Typed asynchronous API definitions exposed to Flutter via `flutter_rust_bridge`.
* **`simple-signal-ui` (Flutter Application):**
  * Provider/Bloc state management communicating with Rust via generated FFI bindings.
  * QR Code rendering and camera scanner modules.
  * Mobile background lifecycle hooks managing Android Foreground Service (`FOREGROUND_SERVICE_REMOTE_MESSAGING`) and iOS background fetch.

### 2. Network Protocol & Wire Formatting
* **Uniform 16KB Blocks:** Every outgoing and incoming frame over Tor WebSocket is padded to exactly 16,384 bytes.
* **SMP Transmission Frame:**
  `[ 1 Byte Count ] [ 2 Bytes Length ] [ Signature ] [ 24 Bytes Nonce/CorrId ] [ QueueId ] [ Command ]`
* **Signal Payload Placement:**
  Ciphertext bytes produced by `libsignal::CiphertextMessage::serialize()` are placed directly into the `sentClientMsgBody` field of the SMP `SEND` command.

### 3. Local Storage Schema (SQLCipher)
* Database page size: 4096 bytes with WAL mode (`PRAGMA journal_mode = WAL;`).
* Tables:
  * `identity_keys`: Curve25519 and Ed25519 identity keypairs.
  * `kyber_prekeys`: Post-quantum ML-KEM-1024 prekey records and signatures.
  * `conversations`: Pairwise queue identifiers (`send_queue_uri`, `receive_queue_id`, `relay_onion_address`, `sender_auth_key`).
  * `sessions`: Serialized `libsignal::SessionRecord` blobs linked to `conversations(id)` with `ON DELETE CASCADE`.
  * `messages`: Message metadata, direction, timestamps, delivery status, and body/attachments.

### 4. Push Notification Blinding Flow
* The client registers an APNs/FCM token with a Notification Router using `TNEW` to obtain a blind `tokenId`.
* The client assigns a decoupled `notifierId` on the SMP relay queue using `NKEY`.
* The client binds the notification trigger using `SNEW <tokenId> <smpRouter> <notifierId> <notifierKey>`.
* On message arrival, the SMP relay signals the Notification Router via `NMSG`, which pings APNs/FCM with an empty wakeup trigger.

---

## Testing Decisions

### 1. Test Philosophy
* Tests must exercise observable system behavior through external boundaries rather than internal function mechanics.
* All cryptographic operations must be tested against standard test vectors and round-trip ping-pong validations.

### 2. Modules to be Tested
* **`crypto::ratchet` (Integration Tests):**
  * Full PQXDH handshake between two simulated participants (Alice and Bob).
  * Double Ratchet ping-pong: Out-of-order message decryption, lost packet recovery, and forward secrecy verification.
* **`net::smp` (Protocol Tests):**
  * Parsing and serialization of 16KB padded transport blocks.
  * Command encoding for `NEW`, `SKEY`, `SEND`, `SUB`, and `ACK`.
* **`storage::db` (Database Tests):**
  * Schema creation, encryption key derivation, and migration tests using SQLCipher test keys.
  * Cascading deletion tests: Verifying that removing a conversation deletes all associated sessions and messages.
* **`net::tor` (Mock & Live Tor Tests):**
  * Circuit dormancy state transitions.
  * Connecting to live mock `.onion` hidden services over Arti.

---

## Out of Scope

* **Real-time WebRTC Audio & Video Calling:** Live voice and video streaming requires UDP and anonymous TURN relay infrastructures; deferred to Phase 2.
* **Multi-Device Real-time Synchronization:** Concurrent queue replication across multiple active devices without central servers is deferred; profile migration is handled via encrypted backup.
* **Public Channels & Groups > 50 members:** Focus is strictly on 1-on-1 private pairwise messaging and small private groups.
* **Clearnet Fallback:** The application deliberately omits direct clearnet connections to prevent accidental IP address leakage.

---

## Further Notes

* **Upstream Dependencies:**
  * Tor Arti: `arti-client` (The Tor Project)
  * Signal Protocol: `libsignal` (Signal Messenger, LLC)
  * Database: `rusqlite` with `bundled-sqlcipher`
  * Mobile Bridge: `flutter_rust_bridge` v2
