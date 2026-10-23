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
/// Returns JSON: {"success": true, "conversation": {...}} or error.
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

    match app.import_contact_link(uri) {
        Ok(conv) => to_c_string(json!({"success": true, "conversation": conv}).to_string()),
        Err(e) => to_c_string(json!({"success": false, "error": e.to_string()}).to_string()),
    }
}

/// Encrypts and sends a message across the Double Ratchet session.
/// Returns JSON: {"success": true, "message": {...}} or error.
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

    match app.send_chat_message(conv_id, text) {
        Ok(msg) => to_c_string(json!({"success": true, "message": msg}).to_string()),
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
