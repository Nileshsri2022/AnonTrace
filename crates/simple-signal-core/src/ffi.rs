use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Mutex;
use serde_json::json;

use crate::api::{SimpleSignalApp, TransportMode};

static APP_INSTANCE: Mutex<Option<SimpleSignalApp>> = Mutex::new(None);

fn to_c_string(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => CString::new("{\"success\":false,\"error\":\"CString conversion error\"}")
            .unwrap()
            .into_raw(),
    }
}

unsafe fn to_rust_str<'a>(ptr: *const c_char) -> Result<&'a str, &'static str> {
    if ptr.is_null() {
        return Err("Null pointer provided");
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map_err(|_| "Invalid UTF-8 string encoding")
}

/// Frees a C-string allocated by the Rust core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { drop(CString::from_raw(ptr)) };
    }
}

/// Initializes or unlocks the database using the Master PIN.
/// Returns JSON: {"success": true} or {"success": false, "error": "..."}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_init(
    db_path_ptr: *const c_char,
    passphrase_ptr: *const c_char,
) -> *mut c_char {
    let db_path = match unsafe { to_rust_str(db_path_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };
    let passphrase = match unsafe { to_rust_str(passphrase_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };

    match SimpleSignalApp::init(db_path, passphrase) {
        Ok(app) => {
            let mut guard = APP_INSTANCE.lock().unwrap();
            *guard = Some(app);
            to_c_string(json!({"success": true}).to_string())
        }
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Returns the local user's blind inbox queue ID (Option B).
/// Returns JSON: {"success": true, "queue_id": "<64 hex chars>"} or error.
#[unsafe(no_mangle)]
pub extern "C" fn simple_signal_get_inbox_queue() -> *mut c_char {
    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };

    let queue_id = app.my_inbox_queue_id();
    to_c_string(json!({"success": true, "queue_id": queue_id}).to_string())
}

/// Processes a raw hex-encoded wire payload received from the HTTPS relay queue.
/// Returns JSON: {"success": true, "message": {...}} or {"success": true, "message": null} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_process_payload(payload_hex_ptr: *const c_char) -> *mut c_char {
    let hex_str = match unsafe { to_rust_str(payload_hex_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };

    let payload = match hex::decode(hex_str) {
        Ok(b) => b,
        Err(e) => return to_c_string(json!({"success": false, "error": format!("Hex decode error: {}", e)}).to_string()),
    };

    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };

    match app.process_incoming_payload(&payload) {
        Ok(maybe_msg) => to_c_string(json!({"success": true, "message": maybe_msg}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Returns the user's standardized Contact Link for QR generation.
/// Returns JSON: {"success": true, "uri": "..."} or {"success": false, "error": "..."}
#[unsafe(no_mangle)]
pub extern "C" fn simple_signal_get_contact_link() -> *mut c_char {
    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };

    match app.get_my_contact_link() {
        Ok(uri) => to_c_string(json!({"success": true, "uri": uri}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Imports a partner's Contact Link URI and establishes a pairwise session.
/// Returns JSON: {"success": true, "conversation": {...}, "partner_queue_id": "...", "handshake_hex": "..."} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_import_contact_link(uri_ptr: *const c_char) -> *mut c_char {
    let uri = match unsafe { to_rust_str(uri_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };

    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };

    match app.import_contact_link_with_handshake(uri) {
        Ok((conv, partner_queue_id, handshake_hex)) => to_c_string(json!({
            "success": true,
            "conversation": conv,
            "partner_queue_id": partner_queue_id,
            "handshake_hex": handshake_hex,
        }).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Encrypts and sends a message across the Double Ratchet session.
/// Returns JSON: {"success": true, "message": {...}, "partner_queue_id": "...", "payload_hex": "..."} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_send_message(
    conv_id_ptr: *const c_char,
    text_ptr: *const c_char,
) -> *mut c_char {
    let conv_id = match unsafe { to_rust_str(conv_id_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };
    let text = match unsafe { to_rust_str(text_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };

    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };

    match app.prepare_outbound_chat_message(conv_id, text) {
        Ok((msg, partner_queue_id, payload_hex)) => to_c_string(json!({
            "success": true,
            "message": msg,
            "partner_queue_id": partner_queue_id,
            "payload_hex": payload_hex,
        }).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Lists all active conversations.
/// Returns JSON: {"success": true, "conversations": [...]} or error.
#[unsafe(no_mangle)]
pub extern "C" fn simple_signal_list_conversations() -> *mut c_char {
    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };

    match app.list_conversations() {
        Ok(list) => to_c_string(json!({"success": true, "conversations": list}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Lists all messages in a conversation.
/// Returns JSON: {"success": true, "messages": [...]} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_list_messages(conv_id_ptr: *const c_char) -> *mut c_char {
    let conv_id = match unsafe { to_rust_str(conv_id_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };

    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };

    match app.list_messages(conv_id) {
        Ok(list) => to_c_string(json!({"success": true, "messages": list}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Polls and decrypts incoming messages from the SMP relay queue.
/// If conv_id_ptr is provided and non-empty, filters for that conversation; otherwise syncs all.
/// Returns JSON: {"success": true, "messages": [...]} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_receive_messages(conv_id_ptr: *const c_char) -> *mut c_char {
    let conv_id = if conv_id_ptr.is_null() {
        ""
    } else {
        match unsafe { to_rust_str(conv_id_ptr) } {
            Ok(s) => s,
            Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
        }
    };

    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };

    match app.receive_incoming_messages(conv_id) {
        Ok(list) => to_c_string(json!({"success": true, "messages": list}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Deletes a single message by ID from local storage.
/// Returns JSON: {"success": true, "deleted": bool} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_delete_message(message_id_ptr: *const c_char) -> *mut c_char {
    let msg_id = match unsafe { to_rust_str(message_id_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };

    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };

    match app.delete_message(msg_id) {
        Ok(deleted) => to_c_string(json!({"success": true, "deleted": deleted}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Updates status of a message record.
/// Returns JSON: {"success": true, "updated": bool} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_update_message_status(
    message_id_ptr: *const c_char,
    status_ptr: *const c_char,
) -> *mut c_char {
    let msg_id = match unsafe { to_rust_str(message_id_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };
    let status = match unsafe { to_rust_str(status_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };

    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };

    match app.update_message_status(msg_id, status) {
        Ok(updated) => to_c_string(json!({"success": true, "updated": updated}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Sets the disappearing message duration in seconds for a conversation.
/// Returns JSON: {"success": true} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_set_disappearing_timer(
    conv_id_ptr: *const c_char,
    duration_seconds: i64,
) -> *mut c_char {
    let conv_id = match unsafe { to_rust_str(conv_id_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };

    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };

    match app.set_disappearing_timer(conv_id, duration_seconds) {
        Ok(_) => to_c_string(json!({"success": true}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Returns a conversation's partner sender-auth key (hex) for relay POSTs.
/// Returns JSON: {"success": true, "sender_auth_hex": "..."} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_get_conversation_sender_auth(
    conv_id_ptr: *const c_char,
) -> *mut c_char {
    let conv_id = match unsafe { to_rust_str(conv_id_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };
    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };
    match app.conversation_sender_auth_hex(conv_id) {
        Some(hex) => to_c_string(json!({"success": true, "sender_auth_hex": hex}).to_string()),
        None => to_c_string(json!({"success": false, "error": "Conversation not found"}).to_string()),
    }
}

/// Sets the transport mode ("normal" or "stealth").
/// Returns JSON: {"success": true, "mode": "..."} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_set_transport_mode(
    mode_ptr: *const c_char,
) -> *mut c_char {
    let mode_str = match unsafe { to_rust_str(mode_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };
    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };
    match TransportMode::from_str(mode_str) {
        Some(mode) => {
            app.set_transport_mode(mode);
            to_c_string(json!({"success": true, "mode": mode.as_str()}).to_string())
        }
        None => to_c_string(json!({"success": false, "error": "Unknown transport mode"}).to_string()),
    }
}

/// Starts the embedded Arti bootstrap in the background (Ticket 26.3).
/// `data_dir_ptr` is the app documents dir from Flutter; `onion_ptr`
/// is the relay onion address, or empty for none.
/// Returns JSON: {"success": true} or error. Poll stealth status for progress.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_start_stealth_bootstrap(
    data_dir_ptr: *const c_char,
    onion_ptr: *const c_char,
) -> *mut c_char {
    let data_dir = match unsafe { to_rust_str(data_dir_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };
    let onion_str = if onion_ptr.is_null() {
        ""
    } else {
        match unsafe { to_rust_str(onion_ptr) } {
            Ok(s) => s,
            Err(_) => return to_c_string(json!({"success": false, "error": "Invalid onion address"}).to_string()),
        }
    };
    let onion = if onion_str.is_empty() {
        None
    } else {
        Some(onion_str.to_string())
    };
    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };
    match app.start_stealth_bootstrap(data_dir, onion) {
        Ok(()) => to_c_string(json!({"success": true}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Starts Stealth bootstrap with plain bridge lines (newline-separated).
/// Returns JSON: {"success": true} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_start_stealth_bootstrap_with_bridges(
    data_dir_ptr: *const c_char,
    onion_ptr: *const c_char,
    bridges_ptr: *const c_char,
) -> *mut c_char {
    let data_dir = match unsafe { to_rust_str(data_dir_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };
    let onion_str = if onion_ptr.is_null() {
        ""
    } else {
        match unsafe { to_rust_str(onion_ptr) } {
            Ok(s) => s,
            Err(_) => return to_c_string(json!({"success": false, "error": "Invalid onion address"}).to_string()),
        }
    };
    let onion = if onion_str.is_empty() { None } else { Some(onion_str.to_string()) };
    let bridges_str = if bridges_ptr.is_null() {
        ""
    } else {
        match unsafe { to_rust_str(bridges_ptr) } {
            Ok(s) => s,
            Err(_) => return to_c_string(json!({"success": false, "error": "Invalid bridges"}).to_string()),
        }
    };
    let bridge_lines: Vec<String> = bridges_str
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };
    match app.start_stealth_bootstrap_with_bridges(data_dir, onion, &bridge_lines) {
        Ok(()) => to_c_string(json!({"success": true}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Returns the current Stealth bootstrap status.
/// Returns JSON: {"success": true, "state": "...", "frac": 0.0, "ready": false, "blocked": null, "onion": null} or error.
#[unsafe(no_mangle)]
pub extern "C" fn simple_signal_get_stealth_status() -> *mut c_char {
    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };
    let status = app.stealth_status();
    let state_str = match &status.state {
        crate::net::tor::StealthState::NotStarted => "not_started",
        crate::net::tor::StealthState::Bootstrapping => "bootstrapping",
        crate::net::tor::StealthState::Ready => "ready",
        crate::net::tor::StealthState::Failed(_) => "failed",
    };
    let failed_msg = match &status.state {
        crate::net::tor::StealthState::Failed(msg) => Some(msg.clone()),
        _ => None,
    };
    to_c_string(
        json!({
            "success": true,
            "state": state_str,
            "frac": status.frac,
            "ready": status.ready_for_traffic,
            "blocked": status.blocked,
            "failed": failed_msg,
            "onion": status.onion_address,
        })
        .to_string(),
    )
}

/// Sends a relay POST over Stealth (onion), blocking the caller.
/// Returns JSON: {"success": true, "body": "..."} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_stealth_send(
    queue_ptr: *const c_char,
    auth_ptr: *const c_char,
    payload_ptr: *const c_char,
) -> *mut c_char {
    let queue_id = match unsafe { to_rust_str(queue_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };
    let auth_hex = match unsafe { to_rust_str(auth_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };
    let payload_hex = match unsafe { to_rust_str(payload_ptr) } {
        Ok(s) => s,
        Err(e) => return to_c_string(json!({"success": false, "error": e}).to_string()),
    };
    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };
    match app.stealth_send(queue_id, auth_hex, payload_hex) {
        Ok(body) => to_c_string(json!({"success": true, "body": body}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Returns the current transport mode.
/// Returns JSON: {"success": true, "mode": "normal"|"stealth"} or error.
#[unsafe(no_mangle)]
pub extern "C" fn simple_signal_get_transport_mode() -> *mut c_char {
    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };
    to_c_string(json!({"success": true, "mode": app.transport_mode().as_str()}).to_string())
}

/// Purges all expired messages across all conversations.
/// Returns JSON: {"success": true, "purged_count": usize} or error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn simple_signal_purge_expired_messages() -> *mut c_char {
    let guard = APP_INSTANCE.lock().unwrap();
    let app = match guard.as_ref() {
        Some(a) => a,
        None => return to_c_string(json!({"success": false, "error": "App not initialized"}).to_string()),
    };

    match app.purge_expired_messages() {
        Ok(count) => to_c_string(json!({"success": true, "purged_count": count}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}


