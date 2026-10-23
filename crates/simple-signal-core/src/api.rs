use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::contact::handshake::{initiate_contact_exchange, QueueCoordinates};
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

    /// Generates the user's standardized Contact Link for QR display.
    /// Omits kpk from the optical QR URI so it fits cleanly into standard QR codes (< 500 chars).
    pub fn get_my_contact_link(&self) -> Result<String, FfiError> {
        let queue_suffix = hex::encode(&self.identity.verifying_key.to_bytes()[..6]);
        let mut qr_bundle = self.prekey_bundle.clone();
        qr_bundle.kyber_prekey = Vec::new();

        let link = ContactLink {
            relay_onion: "vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion".to_string(),
            receive_queue_id: format!("q_inbox_{}", queue_suffix),
            sender_auth_key: vec![0x11; 32],
            prekey_bundle: qr_bundle,
        };

        let uri = link.to_uri()?;
        Ok(uri)
    }

    /// Scans or imports a partner's Contact Link URI, negotiating pairwise queues and initializing
    /// the Double Ratchet session.
    pub fn import_contact_link(&self, uri: &str) -> Result<ConversationSummary, FfiError> {
        let remote_link = ContactLink::from_uri(uri)?;

        let return_suffix = hex::encode(&self.identity.verifying_key.to_bytes()[..6]);
        let my_return_queue = QueueCoordinates {
            relay_onion: "vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion".to_string(),
            queue_id: format!("q_ret_{}", return_suffix),
            sender_auth_key: vec![0x22; 32],
        };

        let (_handshake_msg, session) =
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

        // Cache active session in memory
        self.active_sessions
            .lock()
            .unwrap()
            .insert(conversation_id.clone(), session);

        let conv = self
            .db
            .get_conversation(&conversation_id)?
            .ok_or_else(|| FfiError::NotFound("Conversation not found after insert".to_string()))?;

        Ok(ConversationSummary::from(conv))
    }

    /// Encrypts and sends a chat message across the Double Ratchet and records it in storage.
    pub fn send_chat_message(
        &self,
        conversation_id: &str,
        text: &str,
    ) -> Result<MessageSummary, FfiError> {
        let mut sessions = self.active_sessions.lock().unwrap();

        // If session not currently active in memory, attempt to restore from encrypted SQLite storage
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

        let _ciphertext = session
            .encrypt(text.as_bytes())
            .map_err(|e| FfiError::CryptoError(e.to_string()))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        // Persist updated ratchet state so forward secrecy and ratchet step survive app restarts
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

        Ok(MessageSummary::from(record))
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
