# 08: Privacy-Scrubbed In-App Diagnostic Logging & UI Viewer

**What to build:** An in-memory rolling diagnostic logging pipeline with automated secret scrubbing (redacting Master PINs, secret keys, auth tokens, and session identifiers) combined with an in-app viewer (`DebugLogScreen`) featuring category chip filters, live tailing, and secure clipboard export for transparent on-device troubleshooting.

**Blocked by:** 07: Cross-Platform Flutter UI & FFI Bridge Integration.

**Status:** closed

- [x] Implements `LogScrubber` with deterministic regex patterns replacing user PINs, hex keys, and bearer tokens with redacted placeholders.
- [x] Maintains fixed-size 500-entry in-memory ring buffer in `AppLogger` with zero disk persistence.
- [x] Categorizes logs across distinct subsystems (`BOOT`, `AUTH`, `FFI`, `NET`, `CRYPTO`, `TOR`, `DIAG`).
- [x] Provides interactive `DebugLogScreen` in Flutter UI with multi-category chips and copy-to-clipboard functionality.
- [x] Covers log scrubbing and ring buffer capacity constraints with automated tests.
