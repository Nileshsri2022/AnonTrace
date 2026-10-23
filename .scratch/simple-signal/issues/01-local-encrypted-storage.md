# 01: Local Encrypted Storage & Argon2id Key Derivation

**What to build:** An anonymous user can create and initialize their local persona with an optional Master PIN/Passphrase. The application derives a 256-bit AES database key using Argon2id and mounts an encrypted SQLCipher database that securely persists identity keypairs, prekeys, conversations, and ratchet sessions, guaranteeing automated cascading deletion when contacts are removed.

**Blocked by:** None (can start immediately).

**Status:** completed

- [x] User can initialize an encrypted database with an Argon2id-derived 256-bit AES master key from a PIN/passphrase.
- [x] Attempting to open the database with an invalid passphrase fails securely with an authentication error.
- [x] Database schema initializes all tables (`identity_keys`, `prekeys`, `kyber_prekeys`, `conversations`, `sessions`, `messages`) with WAL mode enabled.
- [x] Deleting a conversation triggers an atomic cascading delete that removes all associated session records and messages with zero remnants on disk.
