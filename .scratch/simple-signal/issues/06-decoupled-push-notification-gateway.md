# 06: Decoupled Push Notification Blinding Gateway

**What to build:** An anonymous mobile device registers an ephemeral blinded device token with a Notification Router and links it to decoupled SMP queue notifiers, enabling empty wake-up push triggers through Apple (APNs) and Google (FCM) without disclosing participant identities, social graphs, or message contents.

**Blocked by:** 04: SimpleX SMP Wire Protocol over Tor.

**Status:** closed

- [x] Registers mobile device tokens with notification routers using `TNEW` and receives opaque blinded token IDs.
- [x] Binds decoupled queue notifiers via `NKEY` on the SMP message relay.
- [x] Connects the subscription via `SNEW` so incoming queue messages emit `NMSG` wakeup triggers.
- [x] Wakes up the mobile application in the background to execute brief Tor message retrieval.
