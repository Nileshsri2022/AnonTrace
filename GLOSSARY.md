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
