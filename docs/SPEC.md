# AnonTrace Formal System Specification

**Version:** 1.0.10  
**Status:** Implemented & Verified  
**Classification:** Zero-Knowledge, Post-Quantum, Metadata-Free Messaging  

---

## 1. System Overview

AnonTrace (historically `simple+signal`) is an anonymous messaging system designed to eliminate metadata leakage, surveillance correlation, and cryptographic compromise from quantum computing adversaries. It pairs Signal's Double Ratchet and PQXDH protocols with SimpleX-style unidirectional queue routing over standard Port 443 HTTPS.

---

## 2. Cryptographic Specifications

### 2.1 Primitives & Parameters
- **Post-Quantum KEM**: ML-KEM-1024 (FIPS 203) with 256-bit post-quantum security margin.
- **Diffie-Hellman**: X25519 (RFC 7748) for classical forward secrecy.
- **Digital Signatures**: Ed25519 (RFC 8032) for identity keys and signed prekey authentication.
- **Key Derivation Function (KDF)**: HKDF-SHA256 (RFC 5869).
- **Master Database KDF**: Argon2id (RFC 9106) with $t=3$ iterations, $m=64\,\text{MB}$ memory cost, and $p=4$ parallelism.
- **Symmetric Cipher**: AES-256-GCM (NIST SP 800-38D) with 96-bit nonces and 128-bit authentication tags.

### 2.2 PQXDH Handshake Protocol
The initial pairwise session is established using post-quantum extended triple Diffie-Hellman with hybrid Kyber encapsulation:

$$\text{SK} = \text{HKDF-Extract}(\text{salt}, \text{DH}_1 \mathbin{\Vert} \text{DH}_2 \mathbin{\Vert} \text{DH}_3 \mathbin{\Vert} \text{SS}_{\text{Kyber}})$$

Where:
- $\text{DH}_1 = \text{ECDH}(\text{IK}_A, \text{SPK}_B)$
- $\text{DH}_2 = \text{ECDH}(\text{EK}_A, \text{IK}_B)$
- $\text{DH}_3 = \text{ECDH}(\text{EK}_A, \text{SPK}_B)$
- $\text{SS}_{\text{Kyber}} = \text{ML-KEM-Decaps}(\text{KPK}_B, \text{CT}_{\text{Kyber}})$

### 2.3 Double Ratchet State Machine
Each active pairwise conversation runs an independent Double Ratchet session:
1. **Symmetric Ratchet (Hash Chain)**: Each message sent or received steps the sending or receiving chain key forward using HKDF-SHA256, deriving ephemeral 256-bit message encryption keys that are zeroized immediately after AEAD encryption or decryption.
2. **Diffie-Hellman Ratchet**: Whenever an exchange turn completes, a new ephemeral X25519 keypair is negotiated, ratcheting the root key and restoring post-compromise security.

---

## 3. Network Transport & Wire Framing

### 3.1 Unidirectional Queue Topology
AnonTrace eliminates central accounts and user addresses. Communication between two users (Alice and Bob) relies on two completely decoupled, independent unidirectional queues:
- **Queue $A \to B$**: Created by Bob, written to by Alice, polled exclusively by Bob.
- **Queue $B \to A$**: Created by Alice, written to by Bob, polled exclusively by Alice.

Neither queue knows or contains the identifier of the other queue.

### 3.2 Standard HTTPS Transport (Port 443)
- **Base Endpoint**: `https://<relay_host>/<queue_id>`
- **Firewall Bypass**: Standard TLS Port 443 routes cleanly through mobile cellular carrier middleboxes (Jio, Airtel, Vi) without packet drop.
- **Payload Privacy**: Zero-Knowledge. Payloads are encrypted end-to-end; the relay sees only randomized queue paths and opaque hex strings.

### 3.3 Wire Framing
| Byte 0 (Type) | Payload Structure |
| :--- | :--- |
| `0x00` (Handshake) | JSON-serialized `ContactHandshakeMessage` containing PQXDH components and return queue coordinates. |
| `0x01` (Chat Message)| 32-Byte Sender Identity Key + AES-256-GCM Double Ratchet Ciphertext. |
| `0x02` (Delivery ACK) | 32-Byte Sender Identity Key + AES-256-GCM Double Ratchet Ciphertext carrying delivery/read receipt. |

---

## 4. Contact Link URI Specification

Contact Links are formatted as standardized SimpleX URIs:

```
simplex:/contact#/?v=1&relay=<relay_onion_or_https>&queue=<queue_id>&auth=<auth_key>&ik=<identity_key>&idhk=<id_dh_key>&spkid=<spk_id>&spk=<spk_key>&sig=<spk_signature>&otpkid=<otpk_id>&otpk=<otpk_key>
```

- **Optical QR Optimization**: The bulky ML-KEM-1024 public key is omitted from the optical QR representation to maintain URI length under 500 characters, enabling instant camera focus across mobile sensors. The Kyber prekey is transmitted over-the-wire inside the initial `0x00` handshake frame.

---

## 5. Storage & Privacy Architecture

- **Database Engine**: Embedded SQLite with Write-Ahead Logging (`WAL` mode).
- **Encryption**: Whole-table application-level encryption using 256-bit AES-GCM keys derived from Argon2id.
- **Volatile Diagnostics**: In-memory 500-slot ring buffer (`AppLogger`) with regex redaction (`LogScrubber`). Zero disk logs.
- **Cascading Deletion**: Removing a conversation triggers atomic database transactions deleting all ratchet session records, prekeys, and messages without lingering unlinked data.
- **Local Message Deletion**: Individual messages can be deleted atomically from SQLite via `simple_signal_delete_message` without network transmission or peer session disruption.

---

## 6. Presentation & Timeline UX Specification

- **Timeline Grouping**: Linear message collections are transformed on the client device into chronological segments divided by centered `Date Header Chips` ("Today", "Yesterday", or formatted calendar date strings).
- **Bubble Timestamps**: Individual message bubbles display local 12-hour formatted timestamps (`hh:mm a`) derived from SQLite creation times.
- **Scroll-to-Bottom FAB**: An automatic floating action button with unread message badge count enables effortless jump-to-latest navigation when reviewing historical conversations.
- **In-Band Quoted Replies**: Messages can reference prior utterances via `[QUOTE:snippet]\ntext` payload encapsulation, rendering contextual quote cards in bubbles with zero metadata leakage to network relays.
- **Client-Side Privacy**: Timeline grouping operates purely in the presentation layer with zero network transmission, zero relay disclosure, and zero alteration of post-quantum cryptographic primitives.

---

## 7. Ephemeral Message TTL & Disappearing Messages

- **Storage Schema**: The `messages` table includes `expires_at INTEGER DEFAULT NULL` and `conversations` includes `disappearing_timer_seconds INTEGER DEFAULT 0`.
- **Ephemeral Purge**: An automatic sweep query (`DELETE FROM messages WHERE expires_at IS NOT NULL AND expires_at <= ?1`) atomically purges expired records from encrypted SQLite storage.
- **Configurable Intervals**: Supports Off (0s), 5 Minutes (300s), 1 Hour (3600s), 24 Hours (86400s), and 7 Days (604800s).
- **Zero-Knowledge Transport**: Transport relays carry zero metadata regarding message TTL; purge operations execute completely client-side in local encrypted storage.

