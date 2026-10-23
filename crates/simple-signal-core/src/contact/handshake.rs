use serde::{Deserialize, Serialize};
use x25519_dalek::StaticSecret;

use crate::contact::link::{ContactLink, ContactLinkError};
use crate::crypto::pqxdh::{initiate_pqxdh, process_pqxdh_initiation, IdentityKeyPair, PqxdhMessage, PreKeySecretBundle};
use crate::crypto::ratchet::DoubleRatchetSession;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueCoordinates {
    pub relay_onion: String,
    pub queue_id: String,
    pub sender_auth_key: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContactHandshakeMessage {
    pub pqxdh: PqxdhMessage,
    pub return_queue: QueueCoordinates,
}

impl ContactHandshakeMessage {
    pub fn to_bytes(&self) -> Result<Vec<u8>, ContactLinkError> {
        serde_json::to_vec(self)
            .map_err(|e| ContactLinkError::OperationFailed(format!("Serialize handshake failed: {}", e)))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ContactLinkError> {
        serde_json::from_slice(bytes)
            .map_err(|e| ContactLinkError::OperationFailed(format!("Deserialize handshake failed: {}", e)))
    }
}

/// Initiates contact exchange from a scanned/imported ContactLink.
/// Performs PQXDH key exchange, establishes an initiator DoubleRatchetSession, and formats
/// the initial handshake message carrying the sender's return queue coordinates.
pub fn initiate_contact_exchange(
    remote_link: &ContactLink,
    my_identity: &IdentityKeyPair,
    my_return_queue: QueueCoordinates,
) -> Result<(ContactHandshakeMessage, DoubleRatchetSession), ContactLinkError> {
    let (pqxdh_msg, master_key) = initiate_pqxdh(my_identity, &remote_link.prekey_bundle)
        .map_err(|e| ContactLinkError::OperationFailed(e.to_string()))?;

    let session = DoubleRatchetSession::new_initiator(&master_key, remote_link.prekey_bundle.signed_prekey);

    let handshake_msg = ContactHandshakeMessage {
        pqxdh: pqxdh_msg,
        return_queue: my_return_queue,
    };

    Ok((handshake_msg, session))
}

/// Accepts an incoming handshake message delivered via an SMP receive queue.
/// Completes the PQXDH key exchange, spins up the receiver DoubleRatchetSession, and returns
/// the peer's return queue coordinates.
pub fn accept_contact_handshake(
    handshake: &ContactHandshakeMessage,
    my_identity: &IdentityKeyPair,
    my_secrets: &PreKeySecretBundle,
) -> Result<(DoubleRatchetSession, QueueCoordinates), ContactLinkError> {
    let master_key = process_pqxdh_initiation(my_identity, my_secrets, &handshake.pqxdh)
        .map_err(|e| ContactLinkError::OperationFailed(e.to_string()))?;

    let signed_prekey_secret = StaticSecret::from(my_secrets.signed_prekey_secret.to_bytes());
    let session = DoubleRatchetSession::new_receiver(&master_key, signed_prekey_secret);

    Ok((session, handshake.return_queue.clone()))
}
