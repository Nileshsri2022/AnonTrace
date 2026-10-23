# 04: SimpleX SMP Wire Protocol over Tor

**What to build:** An anonymous client connects to SimpleX Messaging Protocol (SMP) relay servers over Tor `.onion` WebSocket streams to create unidirectional message queues (`NEW`), secure queues with sender keys (`SKEY`), drop 16KB padded encrypted payloads (`SEND`), poll/subscribe for incoming messages (`SUB`/`GET`), and acknowledge message deletion (`ACK`).

**Blocked by:** 02: PQXDH Post-Quantum Handshake & Double Ratchet Envelope, 03: Embedded Tor (Arti) Hidden Service Transport Client.

**Status:** completed

- [x] Creates a unidirectional receive queue on a remote SMP relay using the `NEW` command and receives assigned Queue IDs.
- [x] Secures the queue using `SKEY` so only authorized senders can deliver messages.
- [x] Drops 16,384-byte Signal encrypted payloads into recipient queues via `SEND`.
- [x] Fetches pending messages via `SUB` / `GET` and issues `ACK` commands to ensure server-side deletion.
