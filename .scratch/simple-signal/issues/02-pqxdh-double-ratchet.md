# 02: PQXDH Post-Quantum Handshake & Double Ratchet Envelope

**What to build:** An anonymous user can generate a post-quantum PreKeyBundle containing ML-KEM-1024 public keys, perform an initial PQXDH key exchange with a contact, and continuously send and receive end-to-end encrypted messages over a Signal Double Ratchet with uniform 16,384-byte traffic padding.

**Blocked by:** 01: Local Encrypted Storage & Argon2id Key Derivation.

**Status:** completed

- [x] Generates valid Signal PreKeyBundles incorporating ML-KEM-1024 post-quantum public keys and signatures.
- [x] Executes a simulated bidirectional PQXDH handshake between two clients, deriving matching Root and Chain keys.
- [x] Double Ratchet encrypts and decrypts sequential and out-of-order messages correctly.
- [x] Every encrypted wire payload is uniformly padded to exactly 16,384 bytes with traffic masking bytes.
