# 05: Contact Link & QR Exchange Module

**What to build:** An anonymous user can generate and share a standardized Contact Link (`simplex:/contact#/?...`) or display it as an interactive QR Code containing their temporary receive queue URI and Signal PQXDH prekey bundle, and scan or import a partner's link to immediately negotiate pairwise queues and boot the Double Ratchet.

**Blocked by:** 04: SimpleX SMP Wire Protocol over Tor.

**Status:** closed

- [x] Generates valid `simplex:/contact` URIs containing onion relay address, queue ID, and PQXDH prekey bundle.
- [x] Parses incoming Contact Links from text strings or camera QR code streams.
- [x] Automatically establishes the return queue and initial handshake message upon link acceptance.
- [x] Creates a new conversation entry in local encrypted storage ready for real-time messaging.
