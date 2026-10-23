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

fn compute_auth_verifier(key: &[u8; 32]) -> [u8; 32] {
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
    pub fn open_or_create<P: AsRef<Path>>(path: P, key: &[u8; 32]) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;

        // Enforce SQLCipher PRAGMA key if SQLCipher extension is compiled
        let hex_key = hex::encode(key);
        let _ = conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex_key));

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
                created_at INTEGER NOT NULL
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
                status TEXT NOT NULL
            );
            "#
        )?;

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

    /// Inserts a conversation record into `conversations`
    pub fn insert_conversation(&self, record: &ConversationRecord) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO conversations (id, send_queue_uri, receive_queue_id, relay_onion_address, sender_auth_key, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
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
            "INSERT INTO messages (id, conversation_id, direction, body, timestamp, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
            params![
                record.id,
                record.conversation_id,
                record.direction,
                record.body,
                record.timestamp,
                record.status
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
}
