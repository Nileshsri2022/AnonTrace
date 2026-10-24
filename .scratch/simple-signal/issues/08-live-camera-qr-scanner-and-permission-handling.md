# 08: Live Camera QR Scanner & Permission Handling

**What to build:** An interactive camera QR code scanner in Flutter leveraging `mobile_scanner`, equipped with camera lifecycle control, auto-focus, camera permission error boundaries, manual clipboard URI fallback, and automatic navigation to the newly imported conversation.

**Blocked by:** 05: Contact Link & QR Exchange Module, 07: Cross-Platform Flutter UI & FFI Bridge Integration.

**Status:** closed

- [x] Integrates `MobileScanner` controller with explicit pause/resume hooks tied to screen lifecycle.
- [x] Adds robust `errorBuilder` to render clear user feedback if camera permissions are denied or unavailable.
- [x] Provides manual clipboard paste field and "Connect" button for testing on emulators and optical scan fallbacks.
- [x] Parses scanned Contact Link URIs, triggers `importContactLink`, and automatically transitions user to the active `ChatScreen`.
- [x] Protects against duplicate rapid scanner trigger events using deduplication flags.
