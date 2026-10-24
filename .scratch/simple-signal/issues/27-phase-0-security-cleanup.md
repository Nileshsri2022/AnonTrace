# 27. Phase 0 Security Cleanup (Stop the Bleeding)

| Property | Value |
| :--- | :--- |
| **Status** | Implemented locally 2026-10-24 — docs first, then code, PR later per user instruction (NOT pushed yet) |
| **Type** | Security fix — removes shipped secrets and secret logging |
| **Target** | `apps/simple_signal_mobile/` only (Flutter/Android, no Rust core change) |
| **Framework Skill** | `ask-matt` (docs-first flow: issue → ADR → tickets → implement) |
| **Related ADR** | `docs/adr/0023-phase-0-security-cleanup.md` |
| **Out of scope** | Relay deployment, Flutter analyze/build, real-device tests, Snowflake/WebTunnel, foreground service, groups/media/backup (see §7) |

---

## 1. Executive Summary

Three Phase 0 problems are still in `main` (verified 2026-10-24, after
PRs #2–#4 merged):

1. **Committed signing key.** `apps/simple_signal_mobile/android/app/anontrace.jks`
   is tracked in git, and `build.gradle.kts:33-35` hardcodes
   `storePassword` / `keyPassword = "anontrace123"`. Anyone with the
   repo can sign a release build as AnonTrace and overwrite a user's
   installed app. Worse, `apps/simple_signal_mobile/android/.gitignore`
   contains `!anontrace.jks` — an explicit exception that forces the
   keystore back into git even though `**/*.jks` is ignored above it.
2. **Plaintext PIN logging.** `lib/main.dart:131` logs
   `Unlocking app with user PIN: $pin`. The master PIN lands in the
   log buffer in plaintext and can leak via a debug export or a log
   inspection.
3. **Fabricated fallback contact link (production path).**
   `lib/src/ffi/bridge_ffi.dart` (the real Android/native path) has an
   `else` fallback in `initApp` that invents a contact link with a
   shared, hardcoded `auth=1111…` key and fixed identity/prekey
   material. If the native library fails to load, the app hands the
   user a link that is not their identity, with keys every install
   shares. A shared link must never be shareable as a real contact.

## 2. Tickets

### Ticket 27.1 — Stop logging the PIN
- `lib/main.dart`: remove `$pin` from the `_unlockApp` log line.
  Log only that an unlock was attempted, never the secret itself.
- Verify: `grep -rn "PIN: \$pin" apps/` returns nothing; no other
  log line interpolates `masterPin` / `pin` (checked in §5).

### Ticket 27.2 — Keystore out of git, signing via key.properties
- `git rm --cached apps/simple_signal_mobile/android/app/anontrace.jks`
  (removes it from tracking, keeps the local file — deletion of the
  local file is the operator's choice, not this PR's).
- `apps/simple_signal_mobile/android/.gitignore`: remove the
  `!anontrace.jks` exception so `**/*.jks` actually ignores keystores.
  `key.properties` was already ignored.
- `build.gradle.kts`: read `key.properties` (Flutter's standard
  `key.properties` pattern) if present and create the `release`
  signing config only from it. If `key.properties` is absent, no
  release signing config is created and no password exists anywhere
  in the repo — a release build without a keystore is unsigned,
  which is fail-visible, not fail-silent.
- Add `apps/simple_signal_mobile/android/key.properties.example`
  (placeholder values only, no real secret) so a new operator can
  create a real `key.properties` in one step.
- **Explicitly NOT done in this PR — history purge + key rotation.**
  Removing the file from HEAD does not remove it from git history.
  The old keystore + password must be treated as compromised forever.
  Purging history (e.g. `git filter-repo`) rewrites every commit
  after the key was added and needs a force-push to `main` plus
  re-cloning by every contributor — that is a destructive operator
  decision, not something to hide inside a normal PR. It is recorded
  here as a required follow-up, with the steps in ADR-0023 §4, and
  it must happen before any production release signed with a new key.
  A new keystore must be generated on the operator's own machine;
  a private signing key must never be generated in, or sent through,
  this VM.

### Ticket 27.3 — Remove the fabricated fallback link (production path)
- `bridge_ffi.dart`: delete the hardcoded fallback contact link
  assignment. When the native library is not loaded,
  `getMyContactLink()` now throws
  `Native core not loaded — no contact link available` instead of
  returning a fake link. Failing loudly is correct here: a contact
  link is an identity, and inventing one silently is worse than
  showing an error.
- **Documented exception — web stub.** `bridge_stub.dart` (web-only
  sandbox bridge, never the Android production path) still carries a
  hardcoded demo link, because the web demo has no native core at
  all and would have no link otherwise. It is marked demo-only in
  the ADR and must never be used for real contacts. Changing it
  would break the web demo without improving Android security, so
  it is deliberately left alone.

## 3. Files changed

- `.scratch/simple-signal/issues/27-phase-0-security-cleanup.md` (this file)
- `docs/adr/0023-phase-0-security-cleanup.md`
- `apps/simple_signal_mobile/lib/main.dart`
- `apps/simple_signal_mobile/lib/src/ffi/bridge_ffi.dart`
- `apps/simple_signal_mobile/android/app/build.gradle.kts`
- `apps/simple_signal_mobile/android/.gitignore`
- `apps/simple_signal_mobile/android/key.properties.example` (new)
- `apps/simple_signal_mobile/android/app/anontrace.jks` (untracked, not deleted locally)

## 4. Verification

- `grep -rn "anontrace123" apps/` → no matches.
- `grep -rn "auth=1111" apps/simple_signal_mobile/lib/src/ffi/bridge_ffi.dart` → no matches.
- `grep -rn "PIN: " apps/simple_signal_mobile/lib/main.dart` → no matches.
- `git status` → `anontrace.jks` shows as deleted from tracking
  (untracked removal), not as a modified file.
- `cargo test --locked` → 55/55 green (no Rust code touched; run
  2026-10-24 to confirm no regression from a Flutter-only change).
- NOT verified here (needs the operator's machine): `flutter analyze`,
  a real release build with a real `key.properties`, and the
  history-purge + rotation follow-up in ADR-0023 §4.

## 5. Secret scan (2026-10-24, before the fix)

- `build.gradle.kts:33,35` — `anontrace123` (both passwords).
- `main.dart:131` — PIN interpolation.
- `bridge_ffi.dart:205` — fabricated fallback link (`auth=1111…`).
- `bridge_stub.dart:23` — demo-only link (exception, see §2 Ticket 27.3).
- No other `storePassword` / `keyPassword` / PIN interpolation found
  by grep in `apps/` at the time of writing.

## 6. What this does NOT fix

- History still contains the old keystore (see Ticket 27.2).
- No new keystore is generated here (operator's machine only).
- No relay deployment, no device testing, no audit.

## 7. Explicitly out of scope

- Relay deployment (Step 2 of the production roadmap).
- Flutter analyze / release build / Argon2id device measurement.
- Real-device 4G / Doze / app-killed verification.
- Snowflake/WebTunnel bridges, foreground service, groups, media,
  backup, Android Keystore/biometric wrapping.
- Any change to PQXDH, Double Ratchet, SQLCipher, or the relay API.
