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
