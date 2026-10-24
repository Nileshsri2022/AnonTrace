use std::path::Path;
use std::sync::{Arc, Mutex};
use argon2::{Algorithm, Argon2, Params, Version};
use hmac::{Hmac, Mac};
use rusqlite::{params, Connection};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

const AUTH_CANARY_CONTEXT: &[u8] = b"simple-signal-storage-auth-canary-v1";

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Invalid key derivation parameters: {0}")]
    InvalidKeyDerivationParams(String),

    #[error("Key derivation failed: {0}")]
    KeyDerivationFailed(String),

    #[error("Database authentication failed: invalid passphrase or corrupted key")]
    AuthenticationFailed,

    #[error("Database operation failed: {0}")]
    DatabaseError(String),
}

impl From<rusqlite::Error> for StorageError {
    fn from(err: rusqlite::Error) -> Self {
        StorageError::DatabaseError(err.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct KeyDerivationParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt: [u8; 16],
}

impl Default for KeyDerivationParams {
    fn default() -> Self {
        Self {
            memory_kib: 64 * 1024, // 64 MiB (RFC 9106 recommendation)
            iterations: 3,
            parallelism: 4,
            salt: [0u8; 16],
        }
    }
}

/// Derives a 256-bit (32-byte) AES master database encryption key from a passphrase using Argon2id.
pub fn derive_key(passphrase: &str, params: &KeyDerivationParams) -> Result<[u8; 32], StorageError> {
    if params.memory_kib < 8 * 1024 {
        return Err(StorageError::InvalidKeyDerivationParams(format!(
            "Memory cost too low: {} KiB (minimum is 8192 KiB)",
            params.memory_kib
        )));
    }

    if params.iterations < 1 {
        return Err(StorageError::InvalidKeyDerivationParams(
            "Iterations count must be at least 1".to_string(),
        ));
    }

    if params.parallelism < 1 {
        return Err(StorageError::InvalidKeyDerivationParams(
            "Parallelism must be at least 1".to_string(),
        ));
    }

    let argon_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(32),
    )
    .map_err(|e| StorageError::InvalidKeyDerivationParams(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), &params.salt, &mut key)
        .map_err(|e| StorageError::KeyDerivationFailed(e.to_string()))?;

    Ok(key)
}

pub fn compute_auth_verifier(key: &[u8; 32]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC can take key of any size");
    mac.update(AUTH_CANARY_CONTEXT);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationRecord {
    pub id: String,
    pub send_queue_uri: String,
    pub receive_queue_id: String,
    pub relay_onion_address: String,
    pub sender_auth_key: Vec<u8>,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRecord {
    pub conversation_id: String,
    pub session_record: Vec<u8>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageRecord {
    pub id: String,
    pub conversation_id: String,
    pub direction: String,
    pub body: Vec<u8>,
    pub timestamp: i64,
    pub status: String,
    pub expires_at: Option<i64>,
}

/// Identity + prekey material as stored in the (SQLCipher-encrypted) DB.
/// Layouts are fixed so load/store round-trip exactly:
/// identity public = verifying_key (32) || dh_public (32),
/// identity private = signing_key (32) || dh_secret (32).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredIdentity {
    pub signing_key: [u8; 32],
    pub dh_secret: [u8; 32],
    pub verifying_key: [u8; 32],
    pub dh_public: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredPrekeys {
    pub signed_prekey_id: u32,
    pub signed_prekey: [u8; 32],
    pub signed_secret: [u8; 32],
    pub signature: [u8; 64],
    pub one_time_prekey_id: u32,
    pub one_time_prekey: [u8; 32],
    pub one_time_secret: [u8; 32],
    pub kyber_public: Vec<u8>,
    pub kyber_secret: Vec<u8>,
}

#[derive(Clone)]
pub struct EncryptedDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl std::fmt::Debug for EncryptedDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptedDatabase").finish()
    }
}

impl EncryptedDatabase {
    /// Opens or creates an encrypted database file at the specified path with the derived master key.
    /// With `bundled-sqlcipher` this is real file-level encryption: a wrong
    /// key fails the probe below and is reported as AuthenticationFailed.
    pub fn open_or_create<P: AsRef<Path>>(path: P, key: &[u8; 32]) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;

        // SQLCipher key — the error must propagate, never be discarded.
        let hex_key = hex::encode(key);
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex_key))?;

        // Probe FIRST, before any other pragma that reads pages: on an
        // existing encrypted file a wrong key makes even sqlite_master
        // unreadable. On a new empty file this succeeds.
        if let Err(e) =
            conn.query_row("SELECT count(*) FROM sqlite_master;", [], |row| row.get::<_, i64>(0))
        {
            let msg = e.to_string();
            if msg.contains("not a database") || msg.contains("encrypted") {
                return Err(StorageError::AuthenticationFailed);
            }
            return Err(StorageError::DatabaseError(msg));
        }

        // Always enforce foreign keys and WAL mode
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;

        // Initialize or verify security header table
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _security_header (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                auth_verifier BLOB NOT NULL
            );"
        )?;

        let expected_verifier = compute_auth_verifier(key);

        let stored_verifier: Option<Vec<u8>> = {
            let mut stmt = conn.prepare("SELECT auth_verifier FROM _security_header WHERE id = 1;")?;
            let mut rows = stmt.query([])?;
            if let Some(row) = rows.next()? {
                Some(row.get(0)?)
            } else {
                None
            }
        };

        if let Some(stored) = stored_verifier {
            if stored != expected_verifier {
                return Err(StorageError::AuthenticationFailed);
            }
        } else {
            // New database: write verifier
            conn.execute(
                "INSERT INTO _security_header (id, auth_verifier) VALUES (1, ?1);",
                params![&expected_verifier[..]],
            )?;
        }

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.initialize_schema()?;

        Ok(db)
    }

    /// Initializes all required tables per SPEC.md
    pub fn initialize_schema(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS identity_keys (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key_alias TEXT UNIQUE NOT NULL,
                public_key BLOB NOT NULL,
                private_key BLOB NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS prekeys (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key_id INTEGER UNIQUE NOT NULL,
                public_key BLOB NOT NULL,
                private_key BLOB NOT NULL,
                signature BLOB,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS kyber_prekeys (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key_id INTEGER UNIQUE NOT NULL,
                public_key BLOB NOT NULL,
                private_key BLOB NOT NULL,
                signature BLOB NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                send_queue_uri TEXT NOT NULL,
                receive_queue_id TEXT NOT NULL,
                relay_onion_address TEXT NOT NULL,
                sender_auth_key BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                disappearing_timer_seconds INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                session_record BLOB NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                direction TEXT NOT NULL,
                body BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                status TEXT NOT NULL,
                expires_at INTEGER DEFAULT NULL
            );

            CREATE TABLE IF NOT EXISTS local_kv (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL
            );
            "#
        )?;

        let _ = conn.execute_batch("ALTER TABLE conversations ADD COLUMN disappearing_timer_seconds INTEGER DEFAULT 0;");
        let _ = conn.execute_batch("ALTER TABLE messages ADD COLUMN expires_at INTEGER DEFAULT NULL;");

        Ok(())
    }

    /// Returns the current journal mode (e.g., "wal")
    pub fn get_journal_mode(&self) -> Result<String, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mode: String = conn.query_row("PRAGMA journal_mode;", [], |row| row.get(0))?;
        Ok(mode)
    }

    /// Lists all user tables currently created in sqlite_master
    pub fn list_tables(&self) -> Result<Vec<String>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_security%';"
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut tables = Vec::new();
        for r in rows {
            tables.push(r?);
        }
        Ok(tables)
    }

    /// Stores (or replaces) the single primary identity keypair.
    pub fn store_identity(&self, identity: &StoredIdentity) -> Result<(), StorageError> {
        let mut public = Vec::with_capacity(64);
        public.extend_from_slice(&identity.verifying_key);
        public.extend_from_slice(&identity.dh_public);
        let mut private = Vec::with_capacity(64);
        private.extend_from_slice(&identity.signing_key);
        private.extend_from_slice(&identity.dh_secret);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO identity_keys (key_alias, public_key, private_key, created_at)
             VALUES ('primary', ?1, ?2, ?3)
             ON CONFLICT(key_alias) DO UPDATE SET
                 public_key = excluded.public_key,
                 private_key = excluded.private_key;",
            params![public, private, now],
        )?;
        Ok(())
    }

    /// Loads the primary identity keypair, if one was stored.
    pub fn load_identity(&self) -> Result<Option<StoredIdentity>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT public_key, private_key FROM identity_keys WHERE key_alias = 'primary' LIMIT 1;",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let public: Vec<u8> = row.get(0)?;
            let private: Vec<u8> = row.get(1)?;
            if public.len() != 64 || private.len() != 64 {
                return Err(StorageError::DatabaseError(
                    "Stored identity has unexpected length".to_string(),
                ));
            }
            let mut verifying_key = [0u8; 32];
            let mut dh_public = [0u8; 32];
            let mut signing_key = [0u8; 32];
            let mut dh_secret = [0u8; 32];
            verifying_key.copy_from_slice(&public[0..32]);
            dh_public.copy_from_slice(&public[32..64]);
            signing_key.copy_from_slice(&private[0..32]);
            dh_secret.copy_from_slice(&private[32..64]);
            Ok(Some(StoredIdentity {
                signing_key,
                dh_secret,
                verifying_key,
                dh_public,
            }))
        } else {
            Ok(None)
        }
    }

    /// Stores (or replaces) the signed + one-time + Kyber prekey material.
    /// Kyber rows require a non-null signature column; the bundle signature
    /// is stored there and ignored on load.
    pub fn store_prekeys(&self, prekeys: &StoredPrekeys) -> Result<(), StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO prekeys (key_id, public_key, private_key, signature, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(key_id) DO UPDATE SET
                 public_key = excluded.public_key,
                 private_key = excluded.private_key,
                 signature = excluded.signature;",
            params![
                prekeys.signed_prekey_id as i64,
                prekeys.signed_prekey.to_vec(),
                prekeys.signed_secret.to_vec(),
                prekeys.signature.to_vec(),
                now
            ],
        )?;
        conn.execute(
            "INSERT INTO prekeys (key_id, public_key, private_key, signature, created_at)
             VALUES (?1, ?2, ?3, NULL, ?4)
             ON CONFLICT(key_id) DO UPDATE SET
                 public_key = excluded.public_key,
                 private_key = excluded.private_key,
                 signature = excluded.signature;",
            params![
                prekeys.one_time_prekey_id as i64,
                prekeys.one_time_prekey.to_vec(),
                prekeys.one_time_secret.to_vec(),
                now
            ],
        )?;
        conn.execute(
            "INSERT INTO kyber_prekeys (key_id, public_key, private_key, signature, created_at)
             VALUES (1, ?1, ?2, ?3, ?4)
             ON CONFLICT(key_id) DO UPDATE SET
                 public_key = excluded.public_key,
                 private_key = excluded.private_key,
                 signature = excluded.signature;",
            params![
                prekeys.kyber_public,
                prekeys.kyber_secret,
                prekeys.signature.to_vec(),
                now
            ],
        )?;
        Ok(())
    }

    /// Loads the prekey material, if it was stored. Returns None if any
    /// of the three rows is missing, so callers regenerate as a set.
    pub fn load_prekeys(&self) -> Result<Option<StoredPrekeys>, StorageError> {
        let conn = self.conn.lock().unwrap();
        // Signed prekeys carry a signature, one-time prekeys do not;
        // scan the table and pick the rows we stored by that marker.
        let mut stmt = conn.prepare("SELECT key_id, public_key, private_key, signature FROM prekeys;")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Option<Vec<u8>>>(3)?,
            ))
        })?;
        let mut signed: Option<(i64, Vec<u8>, Vec<u8>, Option<Vec<u8>>)> = None;
        let mut one_time: Option<(i64, Vec<u8>, Vec<u8>, Option<Vec<u8>>)> = None;
        for r in rows {
            let (id, pubk, privk, sig) = r?;
            if sig.is_some() {
                signed = Some((id, pubk, privk, sig));
            } else {
                one_time = Some((id, pubk, privk, sig));
            }
        }
        let (signed_id, signed_pub, signed_priv, signed_sig) = match signed {
            Some(v) => v,
            None => return Ok(None),
        };
        let (one_time_id, one_time_pub, one_time_priv, _) = match one_time {
            Some(v) => v,
            None => return Ok(None),
        };
        let mut kstmt = conn.prepare(
            "SELECT public_key, private_key FROM kyber_prekeys WHERE key_id = 1 LIMIT 1;",
        )?;
        let mut krows = kstmt.query([])?;
        let (kyber_public, kyber_secret) = match krows.next()? {
            Some(row) => (row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?),
            None => return Ok(None),
        };
        if signed_pub.len() != 32
            || signed_priv.len() != 32
            || one_time_pub.len() != 32
            || one_time_priv.len() != 32
        {
            return Err(StorageError::DatabaseError(
                "Stored prekey has unexpected length".to_string(),
            ));
        }
        let signature_vec = signed_sig.unwrap_or_default();
        if signature_vec.len() != 64 {
            return Err(StorageError::DatabaseError(
                "Stored prekey signature has unexpected length".to_string(),
            ));
        }
        let mut signed_prekey = [0u8; 32];
        let mut signed_secret = [0u8; 32];
        let mut one_time_prekey = [0u8; 32];
        let mut one_time_secret = [0u8; 32];
        let mut signature = [0u8; 64];
        signed_prekey.copy_from_slice(&signed_pub);
        signed_secret.copy_from_slice(&signed_priv);
        one_time_prekey.copy_from_slice(&one_time_pub);
        one_time_secret.copy_from_slice(&one_time_priv);
        signature.copy_from_slice(&signature_vec);
        Ok(Some(StoredPrekeys {
            signed_prekey_id: signed_id as u32,
            signed_prekey,
            signed_secret,
            signature,
            one_time_prekey_id: one_time_id as u32,
            one_time_prekey,
            one_time_secret,
            kyber_public,
            kyber_secret,
        }))
    }

    /// Stores a small local value (e.g. per-inbox queue secret) in local_kv.
    pub fn store_local_value(&self, key: &str, value: &[u8]) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO local_kv (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
            params![key, value],
        )?;
        Ok(())
    }

    /// Loads a local value, if stored.
    pub fn load_local_value(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM local_kv WHERE key = ?1 LIMIT 1;")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    /// Inserts or updates a conversation record in `conversations` idempotently
    pub fn insert_conversation(&self, record: &ConversationRecord) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO conversations (id, send_queue_uri, receive_queue_id, relay_onion_address, sender_auth_key, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                 send_queue_uri = excluded.send_queue_uri,
                 receive_queue_id = excluded.receive_queue_id,
                 relay_onion_address = excluded.relay_onion_address,
                 sender_auth_key = excluded.sender_auth_key;",
            params![
                record.id,
                record.send_queue_uri,
                record.receive_queue_id,
                record.relay_onion_address,
                record.sender_auth_key,
                record.created_at
            ],
        )?;
        Ok(())
    }

    /// Retrieves a conversation record by ID
    pub fn get_conversation(&self, id: &str) -> Result<Option<ConversationRecord>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, send_queue_uri, receive_queue_id, relay_onion_address, sender_auth_key, created_at
             FROM conversations WHERE id = ?1;",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ConversationRecord {
                id: row.get(0)?,
                send_queue_uri: row.get(1)?,
                receive_queue_id: row.get(2)?,
                relay_onion_address: row.get(3)?,
                sender_auth_key: row.get(4)?,
                created_at: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Deletes a conversation by ID, triggering atomic cascading delete of all related sessions and messages
    pub fn delete_conversation(&self, id: &str) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM conversations WHERE id = ?1;", params![id])?;
        Ok(rows > 0)
    }

    /// Inserts a session record into `sessions`
    pub fn insert_session(&self, record: &SessionRecord) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions (conversation_id, session_record, updated_at) VALUES (?1, ?2, ?3);",
            params![record.conversation_id, record.session_record, record.updated_at],
        )?;
        Ok(())
    }

    /// Retrieves the latest session record for a conversation
    pub fn get_latest_session(&self, conversation_id: &str) -> Result<Option<SessionRecord>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT conversation_id, session_record, updated_at
             FROM sessions WHERE conversation_id = ?1
             ORDER BY updated_at DESC, id DESC LIMIT 1;",
        )?;
        let mut rows = stmt.query(params![conversation_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(SessionRecord {
                conversation_id: row.get(0)?,
                session_record: row.get(1)?,
                updated_at: row.get(2)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Counts session records for a conversation
    pub fn count_sessions(&self, conversation_id: &str) -> Result<usize, StorageError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE conversation_id = ?1;",
            params![conversation_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Inserts a message record into `messages`
    pub fn insert_message(&self, record: &MessageRecord) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO messages (id, conversation_id, direction, body, timestamp, status, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
            params![
                record.id,
                record.conversation_id,
                record.direction,
                record.body,
                record.timestamp,
                record.status,
                record.expires_at
            ],
        )?;
        Ok(())
    }

    /// Counts message records for a conversation
    pub fn count_messages(&self, conversation_id: &str) -> Result<usize, StorageError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1;",
            params![conversation_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Deletes a single message record by ID
    pub fn delete_message(&self, message_id: &str) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM messages WHERE id = ?1;", params![message_id])?;
        Ok(rows > 0)
    }

    /// Updates the status of an existing message (e.g. "sent", "delivered", "read")
    pub fn update_message_status(&self, message_id: &str, status: &str) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE messages SET status = ?1 WHERE id = ?2;",
            params![status, message_id],
        )?;
        Ok(rows > 0)
    }

    /// Lists all conversation records ordered by created_at DESC
    pub fn list_conversations(&self) -> Result<Vec<ConversationRecord>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, send_queue_uri, receive_queue_id, relay_onion_address, sender_auth_key, created_at
             FROM conversations ORDER BY created_at DESC;",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ConversationRecord {
                id: row.get(0)?,
                send_queue_uri: row.get(1)?,
                receive_queue_id: row.get(2)?,
                relay_onion_address: row.get(3)?,
                sender_auth_key: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Lists all message records for a conversation ordered by timestamp ASC
    pub fn list_messages(&self, conversation_id: &str) -> Result<Vec<MessageRecord>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, direction, body, timestamp, status, expires_at
             FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC;",
        )?;
        let rows = stmt.query_map(params![conversation_id], |row| {
            Ok(MessageRecord {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                direction: row.get(2)?,
                body: row.get(3)?,
                timestamp: row.get(4)?,
                status: row.get(5)?,
                expires_at: row.get(6)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Sets the disappearing messages duration in seconds for a conversation (0 to disable)
    pub fn set_disappearing_timer(&self, conversation_id: &str, seconds: i64) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE conversations SET disappearing_timer_seconds = ?1 WHERE id = ?2;",
            params![seconds, conversation_id],
        )?;
        Ok(())
    }

    /// Gets the disappearing messages duration in seconds for a conversation
    pub fn get_disappearing_timer(&self, conversation_id: &str) -> Result<i64, StorageError> {
        let conn = self.conn.lock().unwrap();
        let timer: rusqlite::Result<i64> = conn.query_row(
            "SELECT disappearing_timer_seconds FROM conversations WHERE id = ?1;",
            params![conversation_id],
            |row| row.get(0),
        );
        match timer {
            Ok(t) => Ok(t),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
            Err(e) => Err(StorageError::DatabaseError(e.to_string())),
        }
    }

    /// Purges all expired messages whose expires_at is less than or equal to current_timestamp
    pub fn purge_expired_messages(&self, current_timestamp: i64) -> Result<usize, StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM messages WHERE expires_at IS NOT NULL AND expires_at <= ?1;",
            params![current_timestamp],
        )?;
        Ok(rows)
    }
}
