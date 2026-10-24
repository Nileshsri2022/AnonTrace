# 25. Identity Persistence and Real DB Encryption (Option A)

| Property | Value |
| :--- | :--- |
| **Status** | Implemented 2026-10-24 — `cargo test --locked` green (45 tests, 5 new in `identity_persistence_test.rs`) |
| **Type** | Security / correctness fix — local security foundation |
| **Target** | `crates/simple-signal-core` (`storage/db.rs`, `api.rs`, `crypto/pqxdh.rs` — load path only) |
| **Framework Skill** | `ask-matt` (docs-first flow: issue → ADR → tickets → implement) |
| **Related ADR** | `docs/adr/0021-identity-persistence-and-real-db-encryption.md` |
| **Out of scope** | Transport, Tor, relay, Flutter UI, Android Keystore, groups, media, backup (see §7) |

---

## 1. Executive Summary

Option A is the foundation fix for AnonTrace. Two claims in the README are
currently false in the shipped code, and one product behaviour is broken on
every app restart:

1. **Identity is not persisted.** `SimpleSignalApp::init()` in
   `crates/simple-signal-core/src/api.rs` calls `generate_identity_keypair()`
   on every unlock and never reads the `identity_keys` table it creates.
   The prekey bundle and its secrets are likewise regenerated in memory and
   never stored. Result: the contact link (`get_my_contact_link()`) and the
   inbox queue (`my_inbox_queue_id()`, derived from the first 6 bytes of the
   Ed25519 verifying key) change on every restart. Old QR links stop working
   and a returning user is, cryptographically, a new user.
2. **The database is not encrypted at rest.** `EncryptedDatabase::open_or_create()`
   in `storage/db.rs` runs `PRAGMA key = "x'<hex>'";` but discards the result
   (`let _ =`), and `crates/simple-signal-core/Cargo.toml` builds `rusqlite`
   with only the `bundled` feature — no SQLCipher. A plain SQLite reader can
   open the file and read decrypted message bodies, session records, and
   conversation metadata. The stored HMAC auth verifier only gates the app's
   own open path; it does not encrypt the file.
3. **Key derivation is weaker than documented, and the salt is constant.**
   `api.rs` overrides the derivation params to 8 MiB / 1 iteration /
   1 thread with salt `[0x77; 16]` for every user, while
   `KeyDerivationParams::default()` already provides 64 MiB / 3 / 4 and the
   README claims 64 MiB / t=3 / p=4. A constant salt removes per-database
   uniqueness from Argon2id.

This issue fixes exactly those three items, and nothing else. Transport
(Option B: dual HTTPS 443 / Tor mode, private relay) is a separate phase
and must not be mixed into this change.

---

## 2. Current state — evidence in code (v1.0.17, `19ea19f`)

| Claim | Code | Verdict |
| :--- | :--- | :--- |
| Identity persisted | `api.rs`: `let identity = generate_identity_keypair();` on every `init()`; no read of `identity_keys` anywhere in `src/` | False |
| Prekey bundle persisted | `generate_prekey_bundle()` result held only in `SimpleSignalApp` memory (`_prekey_secrets`, `prekey_bundle`); `prekeys` / `kyber_prekeys` tables are created but never written | False |
| DB encrypted | `db.rs`: `let _ = conn.execute_batch(&format!("PRAGMA key = ..."));` + `rusqlite = { features = ["bundled"] }`, no `bundled-sqlcipher` | False — pragma is a no-op |
| Argon2id 64 MiB / t=3 / p=4, random salt | `api.rs`: `memory_kib: 8192, iterations: 1, parallelism: 1, salt: [0x77; 16]` | False — weaker params, constant salt |
| Message bodies encrypted at rest | `api.rs`: `MessageRecord { body: text.as_bytes().to_vec(), .. }` stored directly; inbound plaintext likewise stored after decrypt | Plaintext in the DB file |

No Flutter UI change is required by this issue: the FFI signature
`initApp(dbPath, masterPin)` is unchanged.

---

## 3. Decisions (to be confirmed before code — see ADR-0021)

### D1. Whole-file encryption via SQLCipher (primary approach)
Enable `rusqlite` with `bundled-sqlcipher` and make the existing
`PRAGMA key` call enforced (its error must propagate, not be discarded).
This is the smallest code change that makes the README claim true:
one dependency feature + one enforced pragma, instead of encrypting
every insert/select path by hand.

Fallback, only if the SQLCipher NDK build fails in verification:
per-row envelope encryption (AES-256-GCM under the master key for
`messages.body`, `sessions.session_record`, and private-key columns),
with metadata (ids, timestamps, row counts) explicitly documented as
still plaintext. Do not implement both approaches in the same change.

### D2. Persist identity AND prekey bundle, or the fix is incomplete
Persisting only the identity keypair is not enough: the contact link
also embeds the signed prekey, one-time prekey, and Kyber prekey publics.
If those are regenerated, the link still changes every restart.
Therefore, on first run, generate once and store all three:
- `identity_keys`: one row, alias `primary` — Ed25519 signing key (32 B)
  + X25519 DH secret (32 B) as the private blob; verifying key + DH public
  as the public blob.
- `prekeys`: signed prekey + one-time prekey, public + secret + signature.
- `kyber_prekeys`: ML-KEM-1024 encapsulation key + decapsulation key.

On every later unlock, load all three and reconstruct `IdentityKeyPair`,
`PreKeyBundle`, and `PreKeySecretBundle` from storage. Never regenerate
when a stored row exists.

### D3. Random per-database salt, stored in a sidecar file
New databases: generate a 16-byte salt with `OsRng`, store it as hex
in `<db_path>.salt` (a sidecar file, not a table), and derive with the
default params (64 MiB / 3 / 4). Remove the `8192 / 1 / 1 / [0x77; 16]`
override in `api.rs` entirely.
Correction during implementation: the salt cannot live inside the
encrypted database (it is needed to derive the key that decrypts
that file), so the original draft's `_security_header` / `_kdf_params`
idea was replaced by the sidecar. The salt is public, not secret —
only its uniqueness matters — so a plaintext sidecar is safe.

### D4. Legacy migration, not silent lockout or silent wipe
Existing databases were created with the constant salt and no salt row.
On first unlock after this change:
1. Derive with the legacy constant salt and verify the stored HMAC
   auth verifier (this proves the PIN is correct for the old DB).
2. Generate a new random salt, re-derive, rekey the database to the
   new key (SQLCipher `PRAGMA rekey`), store the new salt, and rewrite
   the auth verifier.
3. Generate-and-persist identity + prekeys once (see D2). This is a
   **breaking change for old QR links**: links shared before the
   upgrade will no longer match the persisted bundle, and affected
   users must re-share their QR once. Sessions and conversations in
   the local DB are preserved; no chat history is deleted by the
   migration. If rekey fails, abort the migration and leave the old
   DB untouched — never half-migrate.

### D5. Android Keystore is explicitly out of scope for this issue
The Rust core only receives `(db_path, masterPin)`; adding Keystore
support would require a Flutter/platform bridge change. SQLCipher with
a PIN-derived key already satisfies the "encrypted at rest" claim.
Keystore-backed key wrapping is defence-in-depth for a later issue,
not a blocker for this one.

---

## 4. Work breakdown (tickets, in dependency order)

### Ticket 25.1 — Salt storage + KDF params + legacy migration path
- Add salt storage as a `<db_path>.salt` sidecar file (see D3).
- `derive_key` callers use the sidecar salt for existing DBs, fresh
  random salt for new DBs.
- `api.rs`: delete the weak override, use `KeyDerivationParams::default()`.
- Implement the D4 legacy migration (verify old verifier → rekey →
  store new salt → rewrite verifier), aborting cleanly on failure.
- Tests: `key_derivation_test` (params + random salt uniqueness),
  `database_lifecycle_test` (legacy DB unlocks with the same PIN after
  upgrade, then unlocks again with the new salt).

### Ticket 25.2 — SQLCipher enablement + enforced pragma
- `Cargo.toml`: `rusqlite` → `features = ["bundled-sqlcipher"]`.
- `db.rs`: propagate the `PRAGMA key` error instead of `let _ =`.
- Verification: open the resulting DB file with a plain SQLite reader —
  it must fail / show no readable tables or message bodies.
- Risk gate: if the multi-arch NDK build fails on SQLCipher, stop and
  return to D1 fallback (envelope encryption) instead of pushing
  through with a broken build.

### Ticket 25.3 — Identity + prekey persistence (load-or-create)
- Add `store_identity` / `load_identity`, `store_prekeys` /
  `load_prekeys`, `store_kyber_prekey` / `load_kyber_prekey` in
  `storage/db.rs` (or a small `storage/identity.rs` if the file grows
  past a surgical diff — prefer keeping it in `db.rs` unless it
  exceeds ~100 new lines).
- `api.rs::init()`: load-or-create identity + prekey bundle + secrets;
  delete the unconditional `generate_identity_keypair()` /
  `generate_prekey_bundle()` calls from the init path (generation
  stays, but only on the create branch).
- Open question for code time (not an assumption): ML-KEM decapsulation
  key serialisation — confirm `fips203` `SerDes` round-trips the
  `DecapsKey` bytes before finalising the kyber storage format.
- Tests: new `identity_persistence_test.rs` — init, drop, re-init with
  the same DB + PIN must yield the identical verifying key, identical
  contact link, identical inbox queue id, and a successful handshake
  with a partner that scanned the pre-restart link.

### Ticket 25.4 — Docs + README correction (this issue's docs phase)
- ADR-0021 (created with this issue, status Proposed → Accepted on
  code merge).
- Update `docs/SPEC.md` / `docs/ARCHITECTURE.md` storage sections and
  the root `README.md` security table only where they currently
  over-claim (SQLCipher, Argon2id params, Tor — Tor stays out of
  scope, but its wording must not be made *more* confident by this
  change).
- No UI copy changes. No transport changes.

---

## 5. Verification criteria (all must pass before merge)

1. `cargo test --locked` — full Rust suite green, including the new
   persistence, migration, and at-rest tests.
2. `flutter test` — green (no UI change expected, but the bridge
   contract must not regress).
3. Restart test: same DB + same PIN across two `init()` calls →
   same identity, same contact link, same inbox queue, old QR link
   still handshakes.
4. At-rest test: the DB file opened without the key yields no
   readable messages, sessions, or private keys.
5. Legacy test: a pre-change DB (constant salt, plaintext file)
   unlocks with the same PIN after upgrade and remains unlockable
   on the next launch.
6. Grep gates: no remaining `[0x77; 16]` salt, no ignored
   `PRAGMA key` result, no unconditional identity generation in
   the init path.
7. Multi-arch APK CI (`build-apk.yml`) passes on a tag build, or the
   SQLCipher fallback in D1 is invoked and this issue is re-scoped.

---

## 6. Risks

- **SQLCipher NDK build failure** — mitigated by the D1 fallback gate;
  do not ship a half-built SQLCipher (e.g. desktop-only) as if it
  covered Android.
- **Legacy migration failure** — mitigated by verify-before-rekey and
  abort-without-touching; a backup copy of the DB before rekey is
  recommended in the implementation, then deleted after a successful
  second unlock.
- **Low-end Android unlock latency** — 64 MiB / t=3 / p=4 is slower
  than 8 MiB / 1 / 1. Measure unlock time on a low-end device in
  verification; if it exceeds ~3 seconds, record the measurement in
  the ADR and revisit params as a separate, explicitly approved
  decision — do not silently weaken them back.
- **Prekey secret format** — ML-KEM decapsulation key serialisation
  is the one format question left open (see Ticket 25.3); resolve it
  in code, not by guessing in this doc.

---

## 7. Explicitly out of scope

- Transport, relay, ntfy replacement, Tor/Arti routing (Option B).
- Android Keystore / biometric key wrapping.
- PIN logging cleanup (one-line fix, separate small issue if wanted).
- Groups, media/attachments, backup export/import, multi-device.
- Any change to the Double Ratchet, PQXDH, or SMP wire formats.
