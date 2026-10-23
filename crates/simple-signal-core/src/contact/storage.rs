use std::time::{SystemTime, UNIX_EPOCH};

use crate::contact::handshake::QueueCoordinates;
use crate::storage::db::{ConversationRecord, EncryptedDatabase, SessionRecord, StorageError};

/// Persists an established contact and initial session record atomically into local encrypted storage.
pub fn persist_contact_conversation(
    db: &EncryptedDatabase,
    conversation_id: &str,
    remote_queue: &QueueCoordinates,
    my_receive_queue_id: &str,
    session_bytes: &[u8],
) -> Result<(), StorageError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let send_queue_uri = format!("simplex://{}/{}", remote_queue.relay_onion, remote_queue.queue_id);

    let conversation = ConversationRecord {
        id: conversation_id.to_string(),
        send_queue_uri,
        receive_queue_id: my_receive_queue_id.to_string(),
        relay_onion_address: remote_queue.relay_onion.clone(),
        sender_auth_key: remote_queue.sender_auth_key.clone(),
        created_at: now,
    };

    let session = SessionRecord {
        conversation_id: conversation_id.to_string(),
        session_record: session_bytes.to_vec(),
        updated_at: now,
    };

    db.insert_conversation(&conversation)?;
    db.insert_session(&session)?;

    Ok(())
}
