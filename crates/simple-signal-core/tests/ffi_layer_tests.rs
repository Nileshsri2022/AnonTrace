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
}
