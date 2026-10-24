# 11: C-ABI Dynamic Library Export & NDK Binding

**What to build:** A lightweight, pure C-ABI FFI layer (`crates/simple-signal-core/src/ffi.rs`) replacing heavy codegen runtime layers, paired with `NativeSimpleSignalBridge` in Flutter utilizing `dart:ffi` to dynamically link `libsimple_signal_core.so` on Android/Linux and `simple_signal_core.dll` on Windows.

**Blocked by:** 07: Cross-Platform Flutter UI & FFI Bridge Integration.

**Status:** closed

- [x] Configures `crate-type = ["cdylib", "rlib"]` in `crates/simple-signal-core/Cargo.toml`.
- [x] Implements `#[no_mangle] pub unsafe extern "C"` functions for app initialization, contact link generation, message sending, and inbox polling.
- [x] Standardizes JSON string serialization and memory management via `simple_signal_free_string`.
- [x] Implements dynamic library loader in `NativeSimpleSignalBridge` detecting Android, Linux, macOS, and Windows platforms.
- [x] Provides unit tests in `crates/simple-signal-core/tests/ffi_layer_tests.rs` verifying C string handling and error reporting.
