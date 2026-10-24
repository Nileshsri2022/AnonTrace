# ADR-0023: Phase 0 Security Cleanup (Stop the Bleeding)

## Status
Accepted (Implemented locally 2026-10-24 — Issue 27; PR later per user
instruction, NOT pushed yet)

## Context
PRs #2–#4 merged the local-security and transport work, but three
shipped secrets / secret-handling bugs survived into `main`:

- The Android release signing keystore is tracked in git with its
  passwords hardcoded in `build.gradle.kts`.
- The master PIN is logged in plaintext on unlock.
- The production native bridge fabricates a contact link with shared
  hardcoded keys when the native library fails to load.

A messenger cannot ship a production release with any of these true:
the first lets anyone impersonate the app's signature, the second
leaks the key that unlocks the encrypted database, and the third
hands users an identity that is not theirs.

This ADR records the decisions for Issue 27, before the code, following
the repo's ask-matt docs-first flow.

## Decisions

1. **No secret ever lives in the repo again.** Signing material comes
   from `key.properties` (Flutter's standard pattern, already
   gitignored) or from CI secrets, never from a hardcoded string and
   never from a tracked keystore file. If `key.properties` is absent,
   the release signing config simply does not exist — an unsigned
   release build is a visible failure, a hardcoded password is a
   silent compromise.

2. **The old keystore is compromised, permanently.** Removing a file
   from HEAD does not un-leak it. The old `anontrace.jks` + password
   must never sign a production release again, even after a history
   purge, because copies of the old history may exist anywhere.

3. **Never log a secret, even in a debug log.** Log lines may record
   that an event happened (unlock attempted, unlock failed), never
   the value of a PIN, key, password, or token. The diagnostic log
   screen in the app makes this stricter than usual: a log line is
   user-visible exportable data, not a private console.

4. **Never fabricate an identity.** If the native core is unavailable,
   `getMyContactLink()` fails loudly instead of returning a plausible
   fake link. A contact link is a cryptographic identity; a shared,
   hardcoded one is worse than no link, because it can be shared,
   imported, and trusted by a real peer.

5. **Web stub is a documented demo-only exception.** `bridge_stub.dart`
   is the web sandbox bridge, not the Android production path, and
   it has no native core to ask for a real link. Its hardcoded demo
   link stays, marked as demo-only, because removing it breaks the
   web demo without improving Android security.

## History purge + rotation (required follow-up, NOT in this PR)

This PR untracks the keystore from HEAD. That alone is not enough
for a production release. The operator must, in order:

1. Generate a **new** keystore on their own machine:
   `keytool -genkeypair -v -keystore new-anontrace.jks -keyalg RSA -keysize 2048 -validity 10000 -alias anontrace`
   Keep it offline / backed up privately; never commit it, never
   send it through a chat, VM, or CI log.
2. Create `android/key.properties` from `key.properties.example`
   with the new keystore's path, alias, and passwords.
3. Purge the old keystore from history (for example with
   `git filter-repo --path apps/simple_signal_mobile/android/app/anontrace.jks --invert-paths`),
   force-push the rewritten `main`, and have every contributor
   re-clone. This rewrites history — it needs an explicit operator
   decision, which is why it is not part of this PR.
4. Treat any app previously signed with the old key as un-updatable
   by the new key: Android will refuse an update signed with a
   different key, so existing installs would need a fresh install.
   That migration cost is a reason to do this purge **before** any
   production release, not after.

## Consequences

- A fresh checkout builds debug fine; a release build without
  `key.properties` is unsigned until the operator adds one.
- No password, PIN, or fabricated identity remains in the Android
  production path (verified by grep in Issue 27 §4).
- The old keystore file still exists locally in the operator's
  working tree until they delete it, and still exists in git
  history until the follow-up purge — both facts are stated here
  so nobody mistakes this PR for a complete rotation.
