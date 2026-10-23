# Rust Core with Flutter UI via flutter_rust_bridge

We decided to implement all core business logic, Tor transport (Arti), cryptography (libsignal), and database management in a unified Rust crate (`simple-signal-core`), exposing typed FFI bindings to a cross-platform Flutter UI using `flutter_rust_bridge`. This achieves zero-overhead memory-safe interoperability with libsignal and Arti while maintaining a single UI codebase for Android, Windows, and Linux.
