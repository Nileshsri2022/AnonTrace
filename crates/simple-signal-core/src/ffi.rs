use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Mutex;
use serde_json::json;

use crate::api::SimpleSignalApp;

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

/// Returns the local user's permanent inbox queue ID.
/// Returns JSON: {"success": true, "queue_id": "anontrace_inbox_..."} or error.
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

