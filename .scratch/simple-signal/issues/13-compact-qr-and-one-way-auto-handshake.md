# 09: Compact Optical QR Code & One-Way Auto-Handshake Flow

**What to build:** An optimized Contact Link optical QR representation under 500 characters ensuring fast camera focus, coupled with an automatic handshake return mechanism (`0x00` frame) allowing messaging immediately after one party scans without requiring reciprocal physical scanning.

**Blocked by:** 05: Contact Link & QR Exchange Module, 07: Cross-Platform Flutter UI & FFI Bridge Integration.

**Status:** closed

- [x] Strips bulky Kyber prekeys from optical QR URI to keep character count under 500 characters for instant mobile camera detection.
- [x] Generates framed Handshake message (`0x00`) carrying sender identity and return queue coordinates upon link import.
- [x] Dispatches handshake message to recipient's inbox queue automatically upon QR scan.
- [x] Receiver automatically discovers incoming handshake, derives post-quantum pairwise session, and populates conversation list.
- [x] Eliminates deadlocks where neither peer could message without a mutual physical scan.
