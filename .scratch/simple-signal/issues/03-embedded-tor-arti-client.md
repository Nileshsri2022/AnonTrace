# 03: Embedded Tor (Arti) Hidden Service Transport Client

**What to build:** The application embeds an in-process Tor client (Arti) that boots natively within the app process without external daemons, resolves and establishes encrypted TLS streams to Tor v3 `.onion` hidden services, enforces per-queue circuit stream isolation, and transitions smoothly into dormant low-power states to preserve mobile battery.

**Blocked by:** None (can start immediately).

**Status:** completed

- [x] In-process Tor client bootstraps consensus and directory metadata without requiring external proxy binaries.
- [x] Connects to Tor v3 `.onion` relay endpoints directly over Tor circuits.
- [x] Enforces stream isolation (`isolate-by-auth` / isolated clients) between distinct contact queues.
- [x] Supports soft dormancy and fast wakeup transitions to conserve mobile device battery.
