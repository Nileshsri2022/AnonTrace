# Simple+Signal Domain Context

The core domain model for an anonymous, end-to-end encrypted messaging application combining Signal cryptographic ratchets with SimpleX unidirectional queue routing over embedded Tor.

## Language

**Pairwise Queue**:
A unidirectional, isolated message queue hosted on an SMP relay allocated specifically for communication between two parties.
_Avoid_: Inbox, mailbox, channel, chat socket

**Contact Link**:
An out-of-band URI containing queue endpoints, certificates, and cryptographic prekey bundles used to establish a duplex connection without user identifiers.
_Avoid_: Invite URL, friend link, user handle, profile link

**Blinded Notification Token**:
An ephemeral, cryptographically hashed token registered with a push notification router that triggers background device wakeup without revealing sender, recipient, or queue identity.
_Avoid_: Device token, APNs token, FCM ID, push token

**Double Ratchet Session**:
An end-to-end encrypted cryptographic state maintaining forward secrecy and post-compromise security via symmetric-key and Diffie-Hellman ratchets.
_Avoid_: Secure tunnel, encryption handshake, chat session

**Relay Onion**:
A Tor v3 hidden service endpoint running an SMP message broker, accessed exclusively through in-process Tor circuits.
_Avoid_: Server, broker host, proxy, clearnet host

**Anonymous Profile**:
A locally stored user persona containing local identity keys and contact linkages, completely decoupled from phone numbers, emails, or central registries.
_Avoid_: Account, user record, login

**Relay Queue**:
An ephemeral, unidirectional message queue hosted on an HTTPS port 443 relay server allocated specifically for post-quantum ciphertext routing across physical devices.
_Avoid_: Chat room, web socket, push channel

**Handshake Frame (0x00)**:
A wire payload carrying PQXDH initialization data, ephemeral keys, and return queue coordinates, enabling automatic contact negotiation upon initial QR scan.
_Avoid_: Friend request, invite token, auth packet

**Encrypted Message Frame (0x01)**:
A wire payload containing the sender verifying key prefix and the Double Ratchet AEAD ciphertext, completely opaque to relay operators and ISPs.
_Avoid_: Cleartext packet, raw message, chat payload

**Privacy-Scrubbed Logger**:
An in-memory rolling diagnostic ring buffer with automated regex redaction of PINs, cryptographic keys, and tokens, maintaining observability without persistent storage exposure.
_Avoid_: System crash log, disk log, debug dump

**SSE Queue Stream**:
A persistent, read-only HTTP chunked connection opened over TLS Port 443 to a unidirectional inbox queue, delivering pushed ciphertexts instantly without repeated TCP/TLS handshakes.
_Avoid_: Polling loop, web socket, long poll

**Unified Message Stream**:
An in-memory broadcast stream within the application layer that dispatches newly decrypted messages simultaneously to conversation lists, notifications, and active chat views, eliminating polling race conditions.
_Avoid_: Database polling, screen refresh loop

**Date Header Chip**:
A centered visual chip rendered within the conversation timeline whenever consecutive messages cross calendar day boundaries, displaying relative labels ("Today", "Yesterday") or localized calendar dates.
_Avoid_: Section title, timestamp label, date banner

**Message Timestamp**:
A formatted local time indicator (`hh:mm a`) placed within the corner of a message bubble denoting the local creation or arrival time of a message without network disclosure.
_Avoid_: Server timestamp, sync time, delivery clock

**Timeline Grouping Adapter**:
A pure client-side transformation module that converts a linear array of raw database messages into an alternating list of date separator chips and message bubble items for the scrollable viewport.
_Avoid_: Chat list sorter, message filter

**Local Message Deletion**:
An atomic cryptographic wipe of a specific individual message record from local encrypted SQLite storage without transmitting network deletion packets or mutating the peer's Double Ratchet session state.
_Avoid_: Remote retract, recall message, server purge

**Scroll-to-Bottom FAB**:
A floating action button rendered within the chat viewport when a user scrolls back into historical messages, displaying an unread badge indicator if incoming messages arrive while scrolled up and providing smooth single-tap navigation back to the live message frontier.
_Avoid_: Jump button, down arrow, scroll helper

**Quoted Reply**:
An end-to-end encrypted message payload that embeds an excerpt of a previous message, rendered in the recipient's viewport as an anchored contextual quote card above the replying text without leaking metadata to transport relays.
_Avoid_: Thread link, message forward, nested post

**Delivery ACK Frame (0x02)**:
A wire payload containing sender verification and a Double Ratchet encrypted acknowledgment packet (`ack_msg_id`), confirming receipt or reading of an earlier message across unidirectional queues without central telemetry.
_Avoid_: Read receipt ping, delivery webhook, heartbeat ack

**Message Status Ticks**:
Visual delivery indicators rendered in outgoing message bubbles denoting state transitions: Single Grey Tick (`sent`), Double Grey Tick (`delivered`), and Double Blue Tick (`read`).
_Avoid_: Status icons, progress dots, sent flags

**Ephemeral Message TTL**:
A time-to-live duration negotiated or configured for a pairwise conversation after which message records and decrypted plaintext are permanently purged from local encrypted storage, leaving zero persistent forensic remnants on physical flash memory.
_Avoid_: Self-destruct timer, auto-delete flag, expiration interval

**Ephemeral Purge Cycle**:
An automated atomic sweep executed against local encrypted storage that deletes all message records whose expiration deadline (`expires_at`) has elapsed relative to local time, preserving zero-knowledge forward secrecy without transmitting network tombstone packets.
_Avoid_: Garbage collection, vacuum job, disk cleanup
