# ADR-0021: Identity Persistence and Real DB Encryption (Option A)

## Status
Accepted (Implemented 2026-10-24 — see issue 25; `cargo test --locked` green, 45 tests including 5 new persistence/migration/at-rest tests)

## Context
AnonTrace is designed as a metadata-free, post-quantum messaging client
combining Signal Double Ratchet with SimpleX unidirectional queues.
A code review of v1.0.17 (`19ea19f`) found that the local-security
foundation does not match the README:

- `SimpleSignalApp::init()` (`crates/simple-signal-core/src/api.rs`)
  generates a fresh identity keypair and prekey bundle on every unlock.
  The `identity_keys`, `prekeys`, and `kyber_prekeys` tables exist in
  the schema but are never written or read. The contact link and the
  inbox queue (derived from the Ed25519 verifying key) therefore
  change on every restart.
- `EncryptedDatabase::open_or_create()` (`storage/db.rs`) issues
  `PRAGMA key` but discards its result, and `rusqlite` is built without
  SQLCipher. The database file is readable with a plain SQLite reader,
  including decrypted message bodies stored as plaintext.
- Key derivation in `api.rs` uses 8 MiB / 1 iteration / 1 thread with
  a constant salt `[0x77; 16]`, while the README claims 64 MiB / t=3 /
  p=4 and `KeyDerivationParams::default()` already provides those
  values.

This ADR records the decisions for Option A (local security first),
before any code is written, following the repo's ask-matt docs-first
flow: issue 25 in `.scratch/` is the working ticket, this ADR is the
durable decision record. Transport work (Option B) is deliberately
excluded here.

## Proposed Decisions & Architecture

1. **Whole-file encryption via SQLCipher.**
   Enable `rusqlite` with the `bundled-sqlcipher` feature and enforce
   the `PRAGMA key` result (propagate its error). This keeps the
   existing storage code paths unchanged and makes the file itself
   unreadable without the key — the property the README already
   claims. If the multi-arch NDK build fails on SQLCipher, fall back
   to per-row envelope encryption (AES-256-GCM under the master key
   for message bodies, session records, and private-key columns) and
   re-scope the issue; do not implement both in one change, and do
   not ship a desktop-only SQLCipher build as if it covered Android.

2. **Persist identity and prekey material together, load-or-create.**
   On first unlock, generate once and store: the identity keypair
   (`identity_keys`, alias `primary`), the signed + one-time prekeys
   (`prekeys`), and the ML-KEM-1024 keypair (`kyber_prekeys`). On
   every later unlock, load all three and reconstruct
   `IdentityKeyPair`, `PreKeyBundle`, and `PreKeySecretBundle` from
   storage. Persisting identity alone is insufficient — the contact
   link also embeds prekey publics, so regenerating prekeys would
   still change the link every restart.

3. **Random per-database salt, default KDF params.**
   New databases get a 16-byte `OsRng` salt stored as hex in a
   `<db_path>.salt` sidecar file. Derivation uses
   `KeyDerivationParams::default()` (64 MiB / 3 / 4); the weak override
   in `api.rs` is deleted, not tuned.
   The salt cannot live inside the encrypted database — it is needed
   to derive the key that decrypts that file — so a sidecar (public,
   uniqueness-only) replaces the earlier `_security_header` idea.

4. **Legacy migration with verify-before-rekey.**
   Databases created before this change have the constant salt and no
   salt row. On first post-upgrade unlock: derive with the legacy
   salt, verify the stored HMAC auth verifier, then generate a new
   random salt, re-derive, rekey the database, store the new salt,
   and rewrite the verifier. Generate-and-persist identity + prekeys
   once at the same migration. If rekey fails, abort and leave the
   old database untouched. Consequence, stated plainly: QR links
   shared before the upgrade will not match the persisted bundle,
   so affected users must re-share their QR once. Local sessions
   and chat history are preserved; nothing is deleted by migration.

5. **Android Keystore is out of scope.**
   The Rust core receives only `(db_path, masterPin)`. Keystore
   wrapping would need a Flutter/platform bridge change and is
   defence-in-depth for a later ADR, not a requirement for the
   "encrypted at rest" claim, which SQLCipher with a PIN-derived
   key already satisfies.

6. **No changes to crypto wire formats or transport.**
   PQXDH, Double Ratchet, SMP framing, relay, and Tor code are
   untouched by this ADR. The FFI signature
   `initApp(dbPath, masterPin)` is unchanged, so no Flutter UI
   change is required.

## Consequences
- Restarting the app preserves identity, contact link, inbox queue,
  and the ability to handshake with partners who scanned the
  pre-restart link. This is the precondition for any real user
  retention, and for Option B's relay authentication, which also
  depends on a stable identity.
- The database file becomes unreadable without the PIN-derived key,
  closing the largest gap between the README's security claims and
  the shipped code.
- One breaking migration (old QR links) is accepted explicitly, in
  exchange for never again regenerating identity silently.
- Unlock latency may rise on low-end Android devices (64 MiB /
  t=3 / p=4 vs the current 8 MiB / 1 / 1). This must be measured
  during verification; any decision to weaken params again must be
  a new, explicitly approved ADR, not a silent revert.
- Open question for implementation (not assumed here): ML-KEM
  decapsulation key serialisation format — confirm the `fips203`
  `SerDes` round-trip before finalising the kyber storage format.

## Verification
See issue 25 §5: full `cargo test --locked`, `flutter test`,
restart-identity test, at-rest readability test, legacy-migration
test, grep gates (no constant salt, no ignored pragma, no
unconditional identity generation), and multi-arch APK CI green.
