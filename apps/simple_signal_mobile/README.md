# AnonTrace Mobile & Web Application

The cross-platform Flutter frontend for **AnonTrace**, providing biometric unlock, live optical QR scanning, real-time Double Ratchet chat, zero-disk privacy diagnostics, and SimpleX-compatible HTTPS Port 443 relay queue communication.

---

## 📱 Features

- **Master PIN & Biometric Screen (`LockScreen`)**: Unlocks the encrypted SQLCipher database via native Rust FFI.
- **Real-Time Double Ratchet Messaging (`ChatScreen`)**: Timelines with optimistic outbound messaging and 1.5s background polling.
- **Optical Camera Scanner (`QrContactScreen`)**: High-performance camera scanner with lifecycle control and manual clipboard fallback.
- **Privacy Diagnostics (`DebugLogScreen`)**: 500-slot in-memory ring buffer with regex scrubbing of PINs and secret keys.
- **Port 443 Transport (`RelayClient`)**: Direct TLS communication over standard HTTPS queues, bypassing cellular firewall restrictions.

---

## 🛠️ Development & Testing

### Running Tests

```bash
flutter test
```

### Running Locally

```bash
# Debug mode on connected physical device or emulator
flutter run -d <device_id>
```

### Compiling NDK JNI Libraries (Before Release Build)

```bash
cd ../../crates/simple-signal-core
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o ../../apps/simple_signal_mobile/android/app/src/main/jniLibs build --release --locked
cd ../../apps/simple_signal_mobile
flutter build apk --release
```
