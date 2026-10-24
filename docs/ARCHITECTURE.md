# AnonTrace System Architecture

AnonTrace (Simple+Signal) is a metadata-free, post-quantum end-to-end encrypted messaging application. It combines Signal's Double Ratchet and PQXDH (ML-KEM-1024 + X25519) cryptographic protocols with SimpleX-style unidirectional ephemeral message queue routing over standard HTTPS Port 443.

---

## 1. High-Level System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Flutter Mobile UI                      │
│   (LockScreen, ChatScreen, QrContactScreen, DebugLogScreen) │
└──────────────┬──────────────────────────────▲───────────────┘
               │                              │
               ▼                              │
┌──────────────────────────────┐              │
│       RelayClient            │              │
│ (dart:io HttpClient, TLS 443)│              │
└──────────────┬───────────────┘              │
               │ (Send payload)               │ (Polled payloads)
               ▼                              │
┌──────────────────────────────┐              │
│   HTTPS Relay (Port 443)     │              │
│   (ntfy.sh / Private Broker) │              │
└──────────────┬───────────────┘              │
               │                              │
               └──────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   Native C-ABI FFI Bridge                   │
│   (simple_signal_init, simple_signal_process_payload, etc.) │
└─────────────────────────────┬───────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   Rust Core (simple-signal-core)            │
│  ┌─────────────────────────┐   ┌──────────────────────────┐ │
│  │     Crypto Engine       │   │     Storage Engine       │ │
│  │ • ML-KEM-1024 (Kyber)   │   │ • SQLite with WAL mode   │ │
│  │ • X25519 & Ed25519      │   │ • AES-256-GCM encryption │ │
│  │ • Double Ratchet        │   │ • Argon2id key derivation│ │
│  │ • HKDF-SHA256           │   │ • Atomic cascading delete│ │
│  └─────────────────────────┘   └──────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Layer Boundaries & Responsibilities

### 2.1 UI Layer (`apps/simple_signal_mobile`)
- **Framework**: Flutter 3.x / Dart 3.x.
- **Views**:
  - `LockScreen`: Enforces Master PIN and biometric authentication before initializing database keys.
  - `ChatScreen`: Renders conversation timelines with instant reactive subscription to `bridge.messageStream`, date-grouped timeline chips ("Today", "Yesterday", calendar dates), and per-message localized timestamps (`hh:mm a`).
  - `QrContactScreen`: Displays compact Contact Link QR codes (< 500 chars) and drives the `MobileScanner` camera feed.
  - `DebugLogScreen`: Live diagnostic viewer with category filters and secret redaction.

### 2.2 Network Transport Layer (`apps/simple_signal_mobile/lib/src/net/relay_client.dart`)
- **Transport**: Standard HTTPS over Port 443.
- **Protocol**: Unidirectional ephemeral queue model with Server-Sent Events (SSE) streaming.
- **Connection Management**:
  - `listenToQueue`: Opens a single, persistent HTTP chunked stream over TLS Port 443 that receives pushed ciphertexts in < 50ms without repeated TCP/TLS handshakes.
  - `sendPayload`: Utilizes persistent keep-alive connections for outbound dispatch, reducing send latency to < 200ms.
  - Automatic reconnection with exponential backoff on 4G cellular tower handoffs or transient drops.
  - Uses native Android system CA trust store, completely avoiding NDK root certificate mismatches.
  - Zero-knowledge transport: The relay server only sees randomly generated queue identifiers and opaque hex ciphertexts.

### 2.3 C-ABI FFI Bridge (`crates/simple-signal-core/src/ffi.rs`)
- Exposes raw C-linkage entrypoints (`extern "C"`) loaded by `dart:ffi`:
  - `simple_signal_init`: Mounts encrypted database using Argon2id master key.
  - `simple_signal_get_contact_link`: Produces optical QR Contact Link URI.
  - `simple_signal_get_inbox_queue`: Returns device's deterministic inbox queue ID.
  - `simple_signal_import_contact_link`: Imports partner URI and returns `(conversation, partner_queue, handshake_hex)`.
  - `simple_signal_send_message`: Encrypts message body and returns `(message, partner_queue, payload_hex)`.
  - `simple_signal_process_payload`: Ingests wire hex payload, executing handshake negotiation or Double Ratchet decryption.
  - `simple_signal_list_conversations` & `simple_signal_list_messages`: Queries persisted records.

### 2.4 Cryptographic Engine (`crates/simple-signal-core/src/crypto/`)
- **Key Derivation**: Argon2id (`t=3, m=64MB, p=4`) derives master 256-bit AES key.
- **Identity & Signatures**: Ed25519 for identity verification and prekey signing.
- **Post-Quantum Key Encapsulation**: ML-KEM-1024 (FIPS 203) combined with X25519 (PQXDH).
- **Session Ratchet**: Double Ratchet with AES-256-GCM symmetric payload cipher and HKDF message keys, providing:
  - **Forward Secrecy**: Compromising a current key cannot decrypt past messages.
  - **Post-Compromise Security**: Continuous DH ratchet advances state to heal compromised sessions.

### 2.5 Storage Engine (`crates/simple-signal-core/src/storage/`)
- **Database**: SQLite with Write-Ahead Logging (`PRAGMA journal_mode = WAL`).
- **Encrypted Tables**: `identity_keys`, `prekeys`, `kyber_prekeys`, `conversations`, `sessions`, `messages`.
- **Cascading Deletion**: Deleting a contact atomically removes all conversation metadata, session ratchet records, and messages.

---

## 3. Wire Framing Protocol

All data dispatched over the HTTPS relay is framed with an 8-bit command header:

### 3.1 Handshake Frame (`0x00`)
```
+---------------+-----------------------------------------------+
| Frame Type    | ContactHandshakeMessage                       |
| 1 Byte (0x00) | (JSON / Protobuf encoded PQXDH initiation)   |
+---------------+-----------------------------------------------+
```
- Dispatched automatically upon scanning a peer's Contact Link QR code.
- Contains initiator's identity key, ephemeral keys, and return queue coordinates.
- Allows receiver to derive matching ratchet keys without a second physical QR scan.

### 3.2 Double Ratchet Encrypted Message Frame (`0x01`)
```
+---------------+-----------------------+-----------------------+
| Frame Type    | Sender Verifying Key  | Double Ratchet Cipher |
| 1 Byte (0x01) | 32 Bytes (Ed25519)    | Variable (AES-256-GCM)|
+---------------+-----------------------+-----------------------+
```
- Dispatched when sending chat messages.
- The 32-byte sender key allows the receiver to identify which active pairwise ratchet session must process the ciphertext.
- Decrypted plaintext is immediately inserted into SQLite and returned to UI.

---

## 4. Threat Model & Security Properties

| Threat Vector | Mitigation Strategy |
| :--- | :--- |
| **Cellular Carrier / ISP Censorship** | Routing over standard Port 443 HTTPS eliminates port filtering common on 4G/5G mobile carriers (Jio, Airtel, Vi). |
| **Relay Operator Snooping** | Zero-knowledge design: Payload bodies are 100% end-to-end encrypted with ML-KEM-1024 & AES-256-GCM. Relays cannot decrypt content. |
| **Physical Device Seizure (Locked)** | Database is encrypted with Argon2id 256-bit AES key; keying material only exists in volatile RAM while unlocked. |
| **Memory / Log Leakage** | `LogScrubber` regex engine strips Master PINs, private keys, and auth headers from all log streams. 500-entry ring buffer is never persisted to disk. |
| **Quantum Adversary ("Harvest Now, Decrypt Later")** | Hybrid PQXDH incorporates FIPS 203 ML-KEM-1024 post-quantum key encapsulation alongside X25519. |
