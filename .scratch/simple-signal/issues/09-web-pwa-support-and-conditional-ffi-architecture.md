# 09: Web PWA Support & Conditional FFI Architecture

**What to build:** Web platform runner files, web manifest (`manifest.json`), PWA icons, automated Vercel deployment build scripts, and a Dart conditional compilation architecture (`bridge.dart`, `bridge_ffi.dart`, `bridge_stub.dart`) allowing seamless browser execution without C FFI crashes.

**Blocked by:** 07: Cross-Platform Flutter UI & FFI Bridge Integration.

**Status:** closed

- [x] Configures `manifest.json`, `index.html`, and PWA service worker metadata under `apps/simple_signal_mobile/web`.
- [x] Implements conditional imports in `bridge.dart` (`if (dart.library.io) 'bridge_ffi.dart' else 'bridge_stub.dart'`).
- [x] Provides sandboxed in-memory cryptographic and messaging state in `bridge_stub.dart` for web browser environments.
- [x] Creates root Vercel build script (`vercel-build.sh`) compiling Flutter web targets cleanly in CI.
- [x] Verifies web application renders lock screen, chat screens, and mock exchanges without runtime exceptions.
