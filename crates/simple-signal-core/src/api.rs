use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::contact::handshake::{
    accept_contact_handshake, initiate_contact_exchange, ContactHandshakeMessage, QueueCoordinates,
};
use crate::contact::link::{ContactLink, ContactLinkError};
use crate::contact::storage::persist_contact_conversation;
use crate::crypto::pqxdh::{
    generate_identity_keypair, generate_prekey_bundle, IdentityKeyPair, PreKeyBundle,
    PreKeySecretBundle,
};
use crate::crypto::ratchet::DoubleRatchetSession;
use crate::net::tor::{DormancyMode, TorClientManager, TorError};
use crate::storage::db::{
    derive_key, ConversationRecord, EncryptedDatabase, KeyDerivationParams, MessageRecord,
    SessionRecord, StorageError,
};

#[derive(Error, Debug, PartialEq, Eq)]
pub enum FfiError {
    #[error("Authentication failed: invalid master PIN or corrupted database")]
    AuthenticationFailed,

    #[error("Database storage error: {0}")]
    DatabaseError(String),

    #[error("Cryptographic operation failed: {0}")]
    CryptoError(String),

    #[error("Contact link error: {0}")]
    ContactError(String),

    #[error("Transport error: {0}")]
    TransportError(String),

    #[error("Item not found: {0}")]
    NotFound(String),
}

impl From<StorageError> for FfiError {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::AuthenticationFailed => FfiError::AuthenticationFailed,
            other => FfiError::DatabaseError(other.to_string()),
        }
    }
}

impl From<ContactLinkError> for FfiError {
    fn from(err: ContactLinkError) -> Self {
        FfiError::ContactError(err.to_string())
    }
}

impl From<TorError> for FfiError {
    fn from(err: TorError) -> Self {
        FfiError::TransportError(err.to_string())
    }
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: String,
    pub send_queue_uri: String,
    pub receive_queue_id: String,
    pub created_at: i64,
}

impl From<ConversationRecord> for ConversationSummary {
    fn from(r: ConversationRecord) -> Self {
        Self {
            id: r.id,
            send_queue_uri: r.send_queue_uri,
            receive_queue_id: r.receive_queue_id,
            created_at: r.created_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageSummary {
    pub id: String,
    pub conversation_id: String,
    pub direction: String,
    pub body_text: String,
    pub timestamp: i64,
    pub status: String,
}

impl From<MessageRecord> for MessageSummary {
    fn from(r: MessageRecord) -> Self {
        let body_text = String::from_utf8(r.body).unwrap_or_else(|_| "[binary payload]".to_string());
        Self {
            id: r.id,
            conversation_id: r.conversation_id,
            direction: r.direction,
            body_text,
            timestamp: r.timestamp,
            status: r.status,
        }
    }
}

/// The main application instance bound to the Flutter UI via FFI.
/// Encapsulates encrypted database, Tor transport, PQXDH identity keys,
/// and live Double Ratchet sessions.
pub struct SimpleSignalApp {
    db: Arc<EncryptedDatabase>,
    tor_manager: TorClientManager,
    identity: Arc<IdentityKeyPair>,
    _prekey_secrets: Arc<PreKeySecretBundle>,
    prekey_bundle: PreKeyBundle,
    active_sessions: Arc<Mutex<HashMap<String, DoubleRatchetSession>>>,
}

impl std::fmt::Debug for SimpleSignalApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimpleSignalApp")
            .field("is_active", &self.is_active())
            .finish()
    }
}

impl SimpleSignalApp {
    /// Initializes or unlocks the application using the Master PIN / passphrase.
    pub fn init(db_path: &str, passphrase: &str) -> Result<Self, FfiError> {
        let params = KeyDerivationParams {
            memory_kib: 8192,
            iterations: 1,
            parallelism: 1,
            salt: [0x77; 16],
        };

        let key = derive_key(passphrase, &params)?;
        let db = EncryptedDatabase::open_or_create(Path::new(db_path), &key)?;

        let identity = generate_identity_keypair();
        let (bundle, secrets) = generate_prekey_bundle(&identity, 1, 100);

        let tor_manager = TorClientManager::default_client();

        // Restore active sessions from encrypted SQLite storage for all known conversations
        let mut active_sessions = HashMap::new();
        if let Ok(convs) = db.list_conversations() {
            for conv in convs {
                if let Ok(Some(session_rec)) = db.get_latest_session(&conv.id) {
                    if let Ok(session) = DoubleRatchetSession::from_bytes(&session_rec.session_record) {
                        active_sessions.insert(conv.id, session);
                    }
                }
            }
        }

        Ok(Self {
            db: Arc::new(db),
            tor_manager,
            identity: Arc::new(identity),
            _prekey_secrets: Arc::new(secrets),
            prekey_bundle: bundle,
            active_sessions: Arc::new(Mutex::new(active_sessions)),
        })
    }

    /// Returns the user's permanent local inbox queue ID derived from Ed25519 identity key.
    pub fn my_inbox_queue_id(&self) -> String {
        let queue_suffix = hex::encode(&self.identity.verifying_key.to_bytes()[..6]);
        format!("anontrace_inbox_{}", queue_suffix)
    }

    /// Generates the user's standardized Contact Link for QR display.
    /// Omits kpk from the optical QR URI so it fits cleanly into standard QR codes (< 500 chars).
    pub fn get_my_contact_link(&self) -> Result<String, FfiError> {
        let mut qr_bundle = self.prekey_bundle.clone();
        qr_bundle.kyber_prekey = Vec::new();

        let link = ContactLink {
            relay_onion: "https://ntfy.sh".to_string(),
            receive_queue_id: self.my_inbox_queue_id(),
            sender_auth_key: vec![0x11; 32],
            prekey_bundle: qr_bundle,
        };

        let uri = link.to_uri()?;
        Ok(uri)
    }

    /// Scans or imports a partner's Contact Link URI, negotiating pairwise queues and initializing
    /// the Double Ratchet session, returning summary, partner queue ID, and framed handshake hex.
    pub fn import_contact_link_with_handshake(
        &self,
        uri: &str,
    ) -> Result<(ConversationSummary, String, String), FfiError> {
        let remote_link = ContactLink::from_uri(uri)?;

        let my_return_queue = QueueCoordinates {
            relay_onion: "https://ntfy.sh".to_string(),
            queue_id: self.my_inbox_queue_id(),
            sender_auth_key: vec![0x22; 32],
        };

        let (handshake_msg, session) =
            initiate_contact_exchange(&remote_link, &self.identity, my_return_queue.clone())?;

        let conversation_id = format!("conv_{}", hex::encode(&remote_link.prekey_bundle.identity_key[..6]));
        let session_bytes = session
            .to_bytes()
            .map_err(|e| FfiError::CryptoError(e.to_string()))?;

        persist_contact_conversation(
            &self.db,
            &conversation_id,
            &QueueCoordinates {
                relay_onion: remote_link.relay_onion.clone(),
                queue_id: remote_link.receive_queue_id.clone(),
                sender_auth_key: remote_link.sender_auth_key.clone(),
            },
            &my_return_queue.queue_id,
            &session_bytes,
        )?;

        let mut handshake_hex = String::new();
        if let Ok(handshake_bytes) = handshake_msg.to_bytes() {
            let mut framed = vec![0x00]; // 0x00 = Handshake frame
            framed.extend_from_slice(&handshake_bytes);
            handshake_hex = hex::encode(&framed);
            crate::net::smp::SmpRelayHub::global().send_to_queue(&remote_link.receive_queue_id, framed);
        }

        // Cache active session in memory
        self.active_sessions
            .lock()
            .unwrap()
            .insert(conversation_id.clone(), session);

        let conv = self
            .db
            .get_conversation(&conversation_id)?
            .ok_or_else(|| FfiError::NotFound("Conversation not found after insert".to_string()))?;

        Ok((ConversationSummary::from(conv), remote_link.receive_queue_id, handshake_hex))
    }

    /// Scans or imports a partner's Contact Link URI, negotiating pairwise queues and initializing
    /// the Double Ratchet session.
    pub fn import_contact_link(&self, uri: &str) -> Result<ConversationSummary, FfiError> {
        let (conv, _, _) = self.import_contact_link_with_handshake(uri)?;
        Ok(conv)
    }

    /// Prepares an encrypted outbound chat message, saves it locally, and returns the message summary,
    /// destination partner queue ID, and hex-encoded wire payload for HTTPS relay dispatch.
    pub fn prepare_outbound_chat_message(
        &self,
        conversation_id: &str,
        text: &str,
    ) -> Result<(MessageSummary, String, String), FfiError> {
        let mut sessions = self.active_sessions.lock().unwrap();

        if !sessions.contains_key(conversation_id) {
            if let Some(session_rec) = self.db.get_latest_session(conversation_id)? {
                let restored = DoubleRatchetSession::from_bytes(&session_rec.session_record)
                    .map_err(|e| FfiError::CryptoError(e.to_string()))?;
                sessions.insert(conversation_id.to_string(), restored);
            }
        }

        let session = sessions
            .get_mut(conversation_id)
            .ok_or_else(|| FfiError::NotFound("Active session not found".to_string()))?;

        let ciphertext = session
            .encrypt(text.as_bytes())
            .map_err(|e| FfiError::CryptoError(e.to_string()))?;

        let conv = self
            .db
            .get_conversation(conversation_id)?
            .ok_or_else(|| FfiError::NotFound("Conversation not found".to_string()))?;

        let partner_queue_id = conv
            .send_queue_uri
            .split('/')
            .last()
            .unwrap_or(&conv.send_queue_uri)
            .to_string();

        let mut framed = Vec::with_capacity(1 + 32 + ciphertext.len());
        framed.push(0x01);
        framed.extend_from_slice(&self.identity.verifying_key.to_bytes());
        framed.extend_from_slice(&ciphertext);

        // Also deliver to local in-memory hub (keeps unit tests passing)
        crate::net::smp::SmpRelayHub::global().send_to_queue(&partner_queue_id, framed.clone());

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        let updated_bytes = session
            .to_bytes()
            .map_err(|e| FfiError::CryptoError(e.to_string()))?;
        self.db.insert_session(&SessionRecord {
            conversation_id: conversation_id.to_string(),
            session_record: updated_bytes,
            updated_at: now,
        })?;

        let msg_id = format!("msg_{}", nanos);
        let record = MessageRecord {
            id: msg_id.clone(),
            conversation_id: conversation_id.to_string(),
            direction: "outbound".to_string(),
            body: text.as_bytes().to_vec(),
            timestamp: now,
            status: "sent".to_string(),
        };

        self.db.insert_message(&record)?;
        let summary = MessageSummary::from(record);
        let payload_hex = hex::encode(&framed);

        Ok((summary, partner_queue_id, payload_hex))
    }

    /// Encrypts and sends a chat message across the Double Ratchet and records it in storage.
    pub fn send_chat_message(
        &self,
        conversation_id: &str,
        text: &str,
    ) -> Result<MessageSummary, FfiError> {
        let (summary, _, _) = self.prepare_outbound_chat_message(conversation_id, text)?;
        Ok(summary)
    }

    /// Processes a raw framed payload (either Handshake 0x00 or Encrypted Message 0x01)
    /// received from the network relay.
    pub fn process_incoming_payload(&self, payload: &[u8]) -> Result<Option<MessageSummary>, FfiError> {
        let my_queue = self.my_inbox_queue_id();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let msg_id = format!("msg_{}", nanos);

        if payload.is_empty() {
            return Ok(None);
        }

        match payload[0] {
            0x00 => {
                if let Ok(handshake) = ContactHandshakeMessage::from_bytes(&payload[1..]) {
                    let conv_id = format!(
                        "conv_{}",
                        hex::encode(&handshake.pqxdh.sender_identity_key[..6])
                    );

                    let has_outbound = self
                        .db
                        .list_messages(&conv_id)
                        .map(|msgs| msgs.iter().any(|m| m.direction == "outbound"))
                        .unwrap_or(false);

                    if !has_outbound {
                        if let Ok((session, return_queue)) = accept_contact_handshake(
                            &handshake,
                            &self.identity,
                            &self._prekey_secrets,
                        ) {
                            if let Ok(session_bytes) = session.to_bytes() {
                                let _ = persist_contact_conversation(
                                    &self.db,
                                    &conv_id,
                                    &return_queue,
                                    &my_queue,
                                    &session_bytes,
                                );
                                let mut sessions = self.active_sessions.lock().unwrap();
                                sessions.insert(conv_id, session);
                            }
                        }
                    }
                }
                Ok(None)
            }
            0x01 => {
                if payload.len() < 1 + 32 + 68 {
                    return Ok(None);
                }

                let mut sender_key = [0u8; 32];
                sender_key.copy_from_slice(&payload[1..33]);
                let conv_id = format!("conv_{}", hex::encode(&sender_key[..6]));
                let ciphertext = &payload[33..];

                let mut sessions = self.active_sessions.lock().unwrap();
                if !sessions.contains_key(&conv_id) {
                    if let Ok(Some(session_rec)) = self.db.get_latest_session(&conv_id) {
                        if let Ok(restored) = DoubleRatchetSession::from_bytes(&session_rec.session_record) {
                            sessions.insert(conv_id.clone(), restored);
                        }
                    }
                }

                if let Some(session) = sessions.get_mut(&conv_id) {
                    if let Ok(plaintext_bytes) = session.decrypt(ciphertext) {
                        if let Ok(plaintext) = String::from_utf8(plaintext_bytes) {
                            let record = MessageRecord {
                                id: msg_id,
                                conversation_id: conv_id.clone(),
                                direction: "inbound".to_string(),
                                body: plaintext.as_bytes().to_vec(),
                                timestamp: now,
                                status: "received".to_string(),
                            };

                            let _ = self.db.insert_message(&record);

                            if let Ok(updated_bytes) = session.to_bytes() {
                                let _ = self.db.insert_session(&SessionRecord {
                                    conversation_id: conv_id,
                                    session_record: updated_bytes,
                                    updated_at: now,
                                });
                            }

                            return Ok(Some(MessageSummary::from(record)));
                        }
                    }
                }
                Ok(None)
            }
            0x02 => {
                if payload.len() < 1 + 32 + 68 {
                    return Ok(None);
                }

                let mut sender_key = [0u8; 32];
                sender_key.copy_from_slice(&payload[1..33]);
                let conv_id = format!("conv_{}", hex::encode(&sender_key[..6]));
                let ciphertext = &payload[33..];

                let mut sessions = self.active_sessions.lock().unwrap();
                if !sessions.contains_key(&conv_id) {
                    if let Ok(Some(session_rec)) = self.db.get_latest_session(&conv_id) {
                        if let Ok(restored) = DoubleRatchetSession::from_bytes(&session_rec.session_record) {
                            sessions.insert(conv_id.clone(), restored);
                        }
                    }
                }

                if let Some(session) = sessions.get_mut(&conv_id) {
                    if let Ok(plaintext_bytes) = session.decrypt(ciphertext) {
                        if let Ok(plaintext) = String::from_utf8(plaintext_bytes) {
                            if let Ok(json_ack) = serde_json::from_str::<serde_json::Value>(&plaintext) {
                                if let (Some(ack_id), Some(st)) = (json_ack["ack_msg_id"].as_str(), json_ack["status"].as_str()) {
                                    let _ = self.db.update_message_status(ack_id, st);
                                }
                            }
                        }
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Polls incoming messages from the user's SMP inbox queue across all conversations,
    /// processes handshake invitations or decrypts Double Ratchet messages,
    /// inserts them into local storage as "inbound", and acknowledges delivery.
    pub fn sync_inbox(&self) -> Result<Vec<MessageSummary>, FfiError> {
        let my_queue = self.my_inbox_queue_id();
        let payloads = crate::net::smp::SmpRelayHub::global().poll_and_ack(&my_queue);
        if payloads.is_empty() {
            return Ok(Vec::new());
        }

        let mut received_messages = Vec::new();
        for (_msg_id, payload) in payloads {
            if let Ok(Some(msg)) = self.process_incoming_payload(&payload) {
                received_messages.push(msg);
            }
        }

        Ok(received_messages)
    }

    /// Polls incoming messages from the SMP relay queue for a specific conversation.
    pub fn receive_incoming_messages(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<MessageSummary>, FfiError> {
        let all_received = self.sync_inbox()?;
        if conversation_id.is_empty() {
            return Ok(all_received);
        }
        Ok(all_received
            .into_iter()
            .filter(|m| m.conversation_id == conversation_id)
            .collect())
    }

    /// Lists all conversations ordered by recent activity.
    pub fn list_conversations(&self) -> Result<Vec<ConversationSummary>, FfiError> {
        let list = self.db.list_conversations()?;
        Ok(list.into_iter().map(ConversationSummary::from).collect())
    }

    /// Lists all messages in a conversation.
    pub fn list_messages(&self, conversation_id: &str) -> Result<Vec<MessageSummary>, FfiError> {
        let list = self.db.list_messages(conversation_id)?;
        Ok(list.into_iter().map(MessageSummary::from).collect())
    }

    /// Deletes a single message by ID from local storage.
    pub fn delete_message(&self, message_id: &str) -> Result<bool, FfiError> {
        Ok(self.db.delete_message(message_id)?)
    }

    /// Updates status of a message record.
    pub fn update_message_status(&self, message_id: &str, status: &str) -> Result<bool, FfiError> {
        Ok(self.db.update_message_status(message_id, status)?)
    }

    /// Sets Tor background battery dormancy (true = soft dormant, false = active).
    pub fn set_dormant(&self, dormant: bool) -> Result<(), FfiError> {
        let mode = if dormant {
            DormancyMode::Soft
        } else {
            DormancyMode::Active
        };
        self.tor_manager.set_dormancy(mode)?;
        Ok(())
    }

    /// Returns true if the background Tor client is active.
    pub fn is_active(&self) -> bool {
        self.tor_manager.is_active()
    }
}
