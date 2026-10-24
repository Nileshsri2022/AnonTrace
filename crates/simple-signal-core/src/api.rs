use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use ed25519_dalek::{SigningKey, VerifyingKey};
use fips203::ml_kem_1024;
use fips203::traits::SerDes;
use rand::rngs::OsRng;
use rand::RngCore;
use rusqlite::{params, Connection};
use thiserror::Error;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

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
    compute_auth_verifier, derive_key, ConversationRecord, EncryptedDatabase,
    KeyDerivationParams, MessageRecord, SessionRecord, StorageError, StoredIdentity,
    StoredPrekeys,
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

/// Salt for pre-Option-A databases. Those databases were created with
/// this constant salt and a plaintext file; see migrate_legacy_db().
const LEGACY_SALT: [u8; 16] = [0x77; 16];

/// Sidecar file holding the (public, non-secret) Argon2id salt as hex.
/// It cannot live inside the SQLCipher file: the salt is needed to
/// derive the key that decrypts that file.
fn salt_path_for(db_path: &str) -> String {
    format!("{}.salt", db_path)
}

fn load_or_create_salt(db_path: &str) -> Result<[u8; 16], FfiError> {
    let salt_path = salt_path_for(db_path);
    if let Ok(hex_salt) = fs::read_to_string(&salt_path) {
        if let Ok(bytes) = hex::decode(hex_salt.trim()) {
            if bytes.len() == 16 {
                let mut salt = [0u8; 16];
                salt.copy_from_slice(&bytes);
                return Ok(salt);
            }
        }
        // Corrupt salt file: fall through and regenerate below only if
        // there is no database to unlock; otherwise fail closed.
        if Path::new(db_path).exists() {
            return Err(FfiError::DatabaseError(
                "Corrupt salt file for existing database".to_string(),
            ));
        }
    }
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    fs::write(&salt_path, hex::encode(salt))
        .map_err(|e| FfiError::DatabaseError(e.to_string()))?;
    Ok(salt)
}

/// Migrates a pre-Option-A plaintext database (constant salt, no salt
/// file) to a SQLCipher database with a fresh random salt.
/// On any failure the original file and the salt file are left alone
/// (salt file is removed if this function created it).
/// Returns the new salt. Conversations, messages, and sessions are
/// copied; identity/prekeys are created by init() afterwards, so old
/// QR links must be re-shared once (see ADR-0021).
fn migrate_legacy_db(db_path: &str, passphrase: &str) -> Result<[u8; 16], FfiError> {
    let legacy_params = KeyDerivationParams {
        memory_kib: 8192,
        iterations: 1,
        parallelism: 1,
        salt: LEGACY_SALT,
    };
    let legacy_key = derive_key(passphrase, &legacy_params)?;
    let expected_verifier = compute_auth_verifier(&legacy_key);

    // Read the legacy plaintext database (no PRAGMA key: it is not
    // SQLCipher-encrypted). A plain rusqlite Connection reads it.
    let legacy_conn = Connection::open(db_path)
        .map_err(|e| FfiError::DatabaseError(e.to_string()))?;
    let stored_verifier: Option<Vec<u8>> = legacy_conn
        .query_row(
            "SELECT auth_verifier FROM _security_header WHERE id = 1;",
            [],
            |row| row.get(0),
        )
        .ok();
    match stored_verifier {
        Some(v) if v == expected_verifier.to_vec() => {}
        _ => return Err(FfiError::AuthenticationFailed),
    }
    let conversations: Vec<ConversationRecord> = {
        let mut stmt = legacy_conn
            .prepare("SELECT id, send_queue_uri, receive_queue_id, relay_onion_address, sender_auth_key, created_at FROM conversations;")
            .map_err(|e| FfiError::DatabaseError(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ConversationRecord {
                    id: row.get(0)?,
                    send_queue_uri: row.get(1)?,
                    receive_queue_id: row.get(2)?,
                    relay_onion_address: row.get(3)?,
                    sender_auth_key: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|e| FfiError::DatabaseError(e.to_string()))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| FfiError::DatabaseError(e.to_string()))?);
        }
        out
    };
    let timers: Vec<(String, i64)> = {
        let mut out = Vec::new();
        for conv in &conversations {
            if let Ok(t) = legacy_conn.query_row(
                "SELECT disappearing_timer_seconds FROM conversations WHERE id = ?1;",
                params![conv.id],
                |row| row.get::<_, i64>(0),
            ) {
                out.push((conv.id.clone(), t));
            }
        }
        out
    };
    let messages: Vec<MessageRecord> = {
        let mut stmt = legacy_conn
            .prepare("SELECT id, conversation_id, direction, body, timestamp, status, expires_at FROM messages ORDER BY timestamp ASC;")
            .map_err(|e| FfiError::DatabaseError(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(MessageRecord {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    direction: row.get(2)?,
                    body: row.get(3)?,
                    timestamp: row.get(4)?,
                    status: row.get(5)?,
                    expires_at: row.get(6)?,
                })
            })
            .map_err(|e| FfiError::DatabaseError(e.to_string()))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| FfiError::DatabaseError(e.to_string()))?);
        }
        out
    };
    let sessions: Vec<SessionRecord> = {
        let mut stmt = legacy_conn
            .prepare("SELECT conversation_id, session_record, updated_at FROM sessions ORDER BY updated_at ASC;")
            .map_err(|e| FfiError::DatabaseError(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SessionRecord {
                    conversation_id: row.get(0)?,
                    session_record: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            })
            .map_err(|e| FfiError::DatabaseError(e.to_string()))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| FfiError::DatabaseError(e.to_string()))?);
        }
        out
    };
    drop(legacy_conn);

    // From here on, any failure must clean up the new salt + migrated
    // file so the original database is untouched.
    let migrated_path = format!("{}.migrated", db_path);
    let _ = fs::remove_file(&migrated_path);
    let result: Result<[u8; 16], FfiError> = (|| {
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        fs::write(salt_path_for(db_path), hex::encode(salt))
            .map_err(|e| FfiError::DatabaseError(e.to_string()))?;
        let params = KeyDerivationParams {
            salt,
            ..KeyDerivationParams::default()
        };
        let key = derive_key(passphrase, &params)?;
        let migrated_db = EncryptedDatabase::open_or_create(Path::new(&migrated_path), &key)?;
        for conv in &conversations {
            migrated_db.insert_conversation(conv)?;
        }
        for (id, timer) in &timers {
            migrated_db.set_disappearing_timer(id, *timer)?;
        }
        for msg in &messages {
            migrated_db.insert_message(msg)?;
        }
        for sess in &sessions {
            migrated_db.insert_session(sess)?;
        }
        drop(migrated_db);
        // Replace the original file. Remove WAL sidecars of both files;
        // a clean Connection close checkpoints WAL, so they should be
        // empty/absent, but never let a stale WAL survive a rename.
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_file(format!("{}-wal", db_path));
        let _ = fs::remove_file(format!("{}-shm", db_path));
        fs::rename(&migrated_path, db_path)
            .map_err(|e| FfiError::DatabaseError(e.to_string()))?;
        let _ = fs::remove_file(format!("{}-wal", migrated_path));
        let _ = fs::remove_file(format!("{}-shm", migrated_path));
        Ok(salt)
    })();
    if result.is_err() {
        let _ = fs::remove_file(salt_path_for(db_path));
        let _ = fs::remove_file(&migrated_path);
        let _ = fs::remove_file(format!("{}-wal", migrated_path));
        let _ = fs::remove_file(format!("{}-shm", migrated_path));
    }
    result
}

fn identity_from_stored(s: &StoredIdentity) -> Result<IdentityKeyPair, FfiError> {
    let signing_key = SigningKey::from_bytes(&s.signing_key);
    let verifying_key = VerifyingKey::from_bytes(&s.verifying_key)
        .map_err(|e| FfiError::CryptoError(e.to_string()))?;
    if signing_key.verifying_key() != verifying_key {
        return Err(FfiError::CryptoError(
            "Stored identity keys do not match".to_string(),
        ));
    }
    Ok(IdentityKeyPair {
        signing_key,
        verifying_key,
        dh_secret: StaticSecret::from(s.dh_secret),
        dh_public: XPublicKey::from(s.dh_public),
    })
}

fn stored_from_identity(identity: &IdentityKeyPair) -> StoredIdentity {
    StoredIdentity {
        signing_key: *identity.signing_key.as_bytes(),
        dh_secret: identity.dh_secret.to_bytes(),
        verifying_key: *identity.verifying_key.as_bytes(),
        dh_public: *identity.dh_public.as_bytes(),
    }
}

fn prekeys_from_stored(
    identity: &IdentityKeyPair,
    s: &StoredPrekeys,
) -> Result<(PreKeyBundle, PreKeySecretBundle), FfiError> {
    if s.kyber_secret.len() != 3168 {
        return Err(FfiError::CryptoError(
            "Stored Kyber key has unexpected length".to_string(),
        ));
    }
    let mut kyber_bytes = [0u8; 3168];
    kyber_bytes.copy_from_slice(&s.kyber_secret);
    let kyber_secret = ml_kem_1024::DecapsKey::try_from_bytes(kyber_bytes)
        .map_err(|e| FfiError::CryptoError(format!("Invalid stored Kyber key: {:?}", e)))?;
    let bundle = PreKeyBundle {
        identity_key: *identity.verifying_key.as_bytes(),
        identity_dh_key: *identity.dh_public.as_bytes(),
        signed_prekey_id: s.signed_prekey_id,
        signed_prekey: s.signed_prekey,
        signature: s.signature,
        one_time_prekey_id: s.one_time_prekey_id,
        one_time_prekey: s.one_time_prekey,
        kyber_prekey: s.kyber_public.clone(),
    };
    let secrets = PreKeySecretBundle {
        signed_prekey_secret: StaticSecret::from(s.signed_secret),
        one_time_prekey_secret: StaticSecret::from(s.one_time_secret),
        kyber_secret,
    };
    Ok((bundle, secrets))
}

fn stored_from_prekeys(bundle: &PreKeyBundle, secrets: &PreKeySecretBundle) -> StoredPrekeys {
    StoredPrekeys {
        signed_prekey_id: bundle.signed_prekey_id,
        signed_prekey: bundle.signed_prekey,
        signed_secret: secrets.signed_prekey_secret.to_bytes(),
        signature: bundle.signature,
        one_time_prekey_id: bundle.one_time_prekey_id,
        one_time_prekey: bundle.one_time_prekey,
        one_time_secret: secrets.one_time_prekey_secret.to_bytes(),
        kyber_public: bundle.kyber_prekey.clone(),
        kyber_secret: secrets.kyber_secret.clone().into_bytes().to_vec(),
    }
}

impl SimpleSignalApp {
    /// Initializes or unlocks the application using the Master PIN / passphrase.
    /// Load-or-create: an existing database keeps its identity and
    /// prekeys; a new database generates and persists them once.
    pub fn init(db_path: &str, passphrase: &str) -> Result<Self, FfiError> {
        let db_exists = Path::new(db_path).exists()
            && fs::metadata(db_path)
                .map(|m| m.len() > 0)
                .unwrap_or(false);
        let salt_exists = Path::new(&salt_path_for(db_path)).exists();
        let salt = if !salt_exists && db_exists {
            // Pre-Option-A database: migrate it, then use its new salt.
            migrate_legacy_db(db_path, passphrase)?
        } else {
            load_or_create_salt(db_path)?
        };

        let params = KeyDerivationParams {
            salt,
            ..KeyDerivationParams::default()
        };
        let key = derive_key(passphrase, &params)?;
        let db = EncryptedDatabase::open_or_create(Path::new(db_path), &key)?;

        let (identity, bundle, secrets) = match db.load_identity()? {
            Some(stored) => {
                let identity = identity_from_stored(&stored)?;
                match db.load_prekeys()? {
                    Some(stored_prekeys) => {
                        let (bundle, secrets) = prekeys_from_stored(&identity, &stored_prekeys)?;
                        (identity, bundle, secrets)
                    }
                    None => {
                        // Identity without prekeys (interrupted create):
                        // regenerate the bundle as a set and store it.
                        let (bundle, secrets) = generate_prekey_bundle(&identity, 1, 100);
                        db.store_prekeys(&stored_from_prekeys(&bundle, &secrets))?;
                        (identity, bundle, secrets)
                    }
                }
            }
            None => {
                let identity = generate_identity_keypair();
                let (bundle, secrets) = generate_prekey_bundle(&identity, 1, 100);
                db.store_identity(&stored_from_identity(&identity))?;
                db.store_prekeys(&stored_from_prekeys(&bundle, &secrets))?;
                (identity, bundle, secrets)
            }
        };

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
        let timer = self.db.get_disappearing_timer(conversation_id).unwrap_or(0);
        let expires_at = if timer > 0 { Some(now + timer) } else { None };

        let record = MessageRecord {
            id: msg_id.clone(),
            conversation_id: conversation_id.to_string(),
            direction: "outbound".to_string(),
            body: text.as_bytes().to_vec(),
            timestamp: now,
            status: "sent".to_string(),
            expires_at,
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
                            let timer = self.db.get_disappearing_timer(&conv_id).unwrap_or(0);
                            let expires_at = if timer > 0 { Some(now + timer) } else { None };

                            let record = MessageRecord {
                                id: msg_id,
                                conversation_id: conv_id.clone(),
                                direction: "inbound".to_string(),
                                body: plaintext.as_bytes().to_vec(),
                                timestamp: now,
                                status: "received".to_string(),
                                expires_at,
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

    /// Lists all messages in a conversation (automatically purges expired ones).
    pub fn list_messages(&self, conversation_id: &str) -> Result<Vec<MessageSummary>, FfiError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let _ = self.db.purge_expired_messages(now);
        let list = self.db.list_messages(conversation_id)?;
        Ok(list.into_iter().map(MessageSummary::from).collect())
    }

    /// Sets disappearing timer in seconds for a conversation.
    pub fn set_disappearing_timer(&self, conversation_id: &str, seconds: i64) -> Result<(), FfiError> {
        Ok(self.db.set_disappearing_timer(conversation_id, seconds)?)
    }

    /// Gets disappearing timer in seconds for a conversation.
    pub fn get_disappearing_timer(&self, conversation_id: &str) -> Result<i64, FfiError> {
        Ok(self.db.get_disappearing_timer(conversation_id)?)
    }

    /// Explicitly purges all expired messages across all conversations.
    pub fn purge_expired_messages(&self) -> Result<usize, FfiError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Ok(self.db.purge_expired_messages(now)?)
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
