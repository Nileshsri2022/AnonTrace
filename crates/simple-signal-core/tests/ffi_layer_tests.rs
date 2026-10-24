use std::ffi::{CStr, CString};
use serde_json::Value;
use tempfile::NamedTempFile;

use simple_signal_core::ffi::*;

#[test]
fn test_ffi_lifecycle() {
    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path().to_str().unwrap();

    let c_db_path = CString::new(db_path).unwrap();
    let c_pin = CString::new("123456").unwrap();

    // 1. Initialize
    let init_res_ptr = unsafe { simple_signal_init(c_db_path.as_ptr(), c_pin.as_ptr()) };
    assert!(!init_res_ptr.is_null());
    let init_str = unsafe { CStr::from_ptr(init_res_ptr).to_str().unwrap() };
    let init_json: Value = serde_json::from_str(init_str).unwrap();
    assert_eq!(init_json["success"], true);
    unsafe { simple_signal_free_string(init_res_ptr) };

    // 2. Get contact link
    let link_res_ptr = simple_signal_get_contact_link();
    assert!(!link_res_ptr.is_null());
    let link_str = unsafe { CStr::from_ptr(link_res_ptr).to_str().unwrap() };
    let link_json: Value = serde_json::from_str(link_str).unwrap();
    assert_eq!(link_json["success"], true);
    let uri = link_json["uri"].as_str().unwrap();
    assert!(uri.starts_with("simplex:/contact#/?v=1&relay="));
    unsafe { simple_signal_free_string(link_res_ptr) };

    // 3. List conversations (should be empty initially)
    let list_res_ptr = simple_signal_list_conversations();
    assert!(!list_res_ptr.is_null());
    let list_str = unsafe { CStr::from_ptr(list_res_ptr).to_str().unwrap() };
    let list_json: Value = serde_json::from_str(list_str).unwrap();
    assert_eq!(list_json["success"], true);
    assert!(list_json["conversations"].as_array().unwrap().is_empty());
    unsafe { simple_signal_free_string(list_res_ptr) };

    // 4. Test receive_messages over FFI (initially empty)
    let c_empty = CString::new("").unwrap();
    let recv_res_ptr = unsafe { simple_signal_receive_messages(c_empty.as_ptr()) };
    assert!(!recv_res_ptr.is_null());
    let recv_str = unsafe { CStr::from_ptr(recv_res_ptr).to_str().unwrap() };
    let recv_json: Value = serde_json::from_str(recv_str).unwrap();
    assert_eq!(recv_json["success"], true);
    assert!(recv_json["messages"].as_array().unwrap().is_empty());
    unsafe { simple_signal_free_string(recv_res_ptr) };

    // 5. Test simple_signal_get_inbox_queue
    let queue_res_ptr = simple_signal_get_inbox_queue();
    assert!(!queue_res_ptr.is_null());
    let queue_str = unsafe { CStr::from_ptr(queue_res_ptr).to_str().unwrap() };
    let queue_json: Value = serde_json::from_str(queue_str).unwrap();
    assert_eq!(queue_json["success"], true);
    assert!(queue_json["queue_id"].as_str().unwrap().starts_with("anontrace_inbox_"));
    unsafe { simple_signal_free_string(queue_res_ptr) };

    // 6. Test simple_signal_process_payload with empty payload
    let c_empty_hex = CString::new("").unwrap();
    let proc_res_ptr = unsafe { simple_signal_process_payload(c_empty_hex.as_ptr()) };
    assert!(!proc_res_ptr.is_null());
    let proc_str = unsafe { CStr::from_ptr(proc_res_ptr).to_str().unwrap() };
    let proc_json: Value = serde_json::from_str(proc_str).unwrap();
    assert_eq!(proc_json["success"], true);
    assert_eq!(proc_json["message"], Value::Null);
    unsafe { simple_signal_free_string(proc_res_ptr) };

    // 7. Test simple_signal_process_payload with invalid hex
    let c_invalid_hex = CString::new("not_hex!").unwrap();
    let err_res_ptr = unsafe { simple_signal_process_payload(c_invalid_hex.as_ptr()) };
    assert!(!err_res_ptr.is_null());
    let err_str = unsafe { CStr::from_ptr(err_res_ptr).to_str().unwrap() };
    let err_json: Value = serde_json::from_str(err_str).unwrap();
    assert_eq!(err_json["success"], false);
    assert!(err_json["error"].as_str().unwrap().contains("Hex decode error"));
    unsafe { simple_signal_free_string(err_res_ptr) };
}
