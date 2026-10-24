# AnonTrace 🛡️

**Metadata-Free, Post-Quantum End-to-End Encrypted Messenger**  
*Combining Signal Cryptographic Ratchets with SimpleX Unidirectional Ephemeral Queues.*

[![Build & Release Android APK](https://github.com/Nileshsri2022/AnonTrace/actions/workflows/build-apk.yml/badge.svg)](https://github.com/Nileshsri2022/AnonTrace/actions/workflows/build-apk.yml)
[![Version](https://img.shields.io/badge/version-1.0.10-blue.svg)](https://github.com/Nileshsri2022/AnonTrace/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Security: Post--Quantum](https://img.shields.io/badge/Security-ML--KEM--1024-purple.svg)](docs/SPEC.md)

---

## 🌟 Overview

**AnonTrace** (previously `simple+signal`) is an anonymous mobile and web messaging application designed to achieve total surveillance resistance. It eliminates phone numbers, user handles, central servers, and metadata leakage by combining **Signal's PQXDH and Double Ratchet** protocols with **SimpleX-style unidirectional queue routing over standard HTTPS Port 443**.

Neither network operators (ISPs), cellular middleboxes, nor relay operators can determine who is communicating with whom, when they are communicating, or the contents of their messages.

---

## ✨ Key Features

- **🚫 No Phone Numbers or User IDs**: Connections are established out-of-band via compact QR codes containing cryptographic prekey bundles.
- **⚛️ Post-Quantum Forward Secrecy**: Hybrid PQXDH handshake pairing **ML-KEM-1024 (FIPS 203)** with **X25519**, immunizing communications against "Harvest Now, Decrypt Later" quantum attacks.
- **🔄 Double Ratchet AEAD Encryption**: Continuous Diffie-Hellman and symmetric-key ratchets with AES-256-GCM ensure past and future message confidentiality.
- **🌐 SimpleX-Compatible Ephemeral Queues over Port 443**: Routes ciphertexts across completely decoupled, unidirectional HTTPS queues over TLS Port 443, effortlessly bypassing carrier-grade firewalls (such as Jio, Airtel, and Vi 4G/5G).
- **📷 Rapid Optical QR Exchange**: Contact Link URIs are optimized to under 500 characters, enabling near-instant camera focus and one-way automated handshake negotiation.
- **🔒 Encrypted SQLite Storage**: Local data is encrypted with 256-bit AES keys derived using Argon2id (`t=3, m=64MB, p=4`), with atomic cascading deletion on contact removal.
- **🔍 Zero-Disk Privacy-Scrubbed Logging**: Built-in 500-slot diagnostic ring buffer with automated regex redaction of PINs, private keys, and auth tokens.
- **📱 Android Foreground Service**: Background remote messaging receiver ensuring reliable delivery without Google Play Services or Firebase Cloud Messaging dependencies.

---

## 🏗️ System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Flutter Mobile UI                      │
│   (LockScreen, ChatScreen, QrContactScreen, DebugLogScreen) │
└──────────────┬──────────────────────────────▲───────────────┘
               │                              │
               ▼                              │
┌──────────────────────────────┐              │
│       RelayClient            │              │
│ (dart:io HttpClient, TLS 443)│              │
└──────────────┬───────────────┘              │
               │ (Send payload)               │ (Polled payloads)
               ▼                              │
┌──────────────────────────────┐              │
│   HTTPS Relay (Port 443)     │              │
│   (ntfy.sh / Private Broker) │              │
└──────────────┬───────────────┘              │
               │                              │
               └──────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   Native C-ABI FFI Bridge                   │
│   (simple_signal_init, simple_signal_process_payload, etc.) │
└─────────────────────────────┬───────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   Rust Core (simple-signal-core)            │
│  ┌─────────────────────────┐   ┌──────────────────────────┐ │
│  │     Crypto Engine       │   │     Storage Engine       │ │
│  │ • ML-KEM-1024 (Kyber)   │   │ • SQLite with WAL mode   │ │
│  │ • X25519 & Ed25519      │   │ • AES-256-GCM encryption │ │
│  │ • Double Ratchet        │   │ • Argon2id key derivation│ │
│  │ • HKDF-SHA256           │   │ • Atomic cascading delete│ │
│  └─────────────────────────┘   └──────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

For complete architecture details, read the [Architecture Specification](docs/ARCHITECTURE.md) and [System Specification](docs/SPEC.md).

---

## 📁 Repository Structure

```
.
├── apps/
│   └── simple_signal_mobile/    # Flutter cross-platform UI (Android, Web, Desktop)
│       ├── lib/src/
│       │   ├── ffi/             # C-ABI FFI bridge & platform-specific dynamic loaders
│       │   ├── logging/         # Zero-disk privacy scrubber and memory logger
│       │   ├── net/             # SimpleX-compatible HTTPS Port 443 RelayClient
│       │   └── screens/         # LockScreen, ChatScreen, QR, and Diagnostics UI
│       └── test/                # Flutter unit, widget, and transport integration tests
├── crates/
│   └── simple-signal-core/      # High-performance Rust cryptographic & storage core
│       ├── src/
│       │   ├── api.rs           # High-level SimpleSignalApp facade
│       │   ├── contact.rs       # ContactLink codec & PQXDH handshake negotiation
│       │   ├── crypto/          # Double Ratchet, ML-KEM-1024, X25519, Argon2id
│       │   ├── ffi.rs           # C-ABI export layer (simple_signal_*)
│       │   ├── net/             # Tor Arti client & SMP wire protocol
│       │   └── storage/         # SQLite schema, migrations, and cascading delete
│       └── tests/               # 34 integration tests (crypto, ratchet, storage, FFI)
├── docs/
│   ├── ARCHITECTURE.md          # End-to-end layered architecture and framing specs
│   ├── SPEC.md                  # Cryptographic parameters and state machines
│   └── adr/                     # Architectural Decision Records (ADR 0001 - 0010)
├── .scratch/simple-signal/      # Feature tickets & issue tracking (Issues 01 - 14)
├── GLOSSARY.md                  # Project domain dictionary and terminology
└── .github/workflows/           # Automated multi-arch Android APK CI/CD pipeline
```

---

## 🚀 Getting Started

### Prerequisites

- **Rust**: 1.78+ (`rustup default stable`)
- **Flutter**: 3.22+ (`flutter doctor`)
- **Android NDK**: r25c+ (for compiling mobile Android binaries)
- **cargo-ndk**: `cargo install cargo-ndk --locked`

### 1. Run Rust Core Tests

```bash
cargo test --locked
```
*All 34 tests covering key derivation, PQXDH, Double Ratchet, SQLite storage, and FFI wire layers should pass.*

### 2. Run Flutter Mobile Tests

```bash
cd apps/simple_signal_mobile
flutter pub get
flutter test
```

### 3. Build Android Release APK

To compile the multi-architecture native libraries and build a release APK locally:

```bash
# 1. Compile Rust core for Android architectures
cd crates/simple-signal-core
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o ../../apps/simple_signal_mobile/android/app/src/main/jniLibs build --release --locked

# 2. Build the signed release APK
cd ../../apps/simple_signal_mobile
flutter build apk --release
```

Output APK will be located at:  
`apps/simple_signal_mobile/build/app/outputs/flutter-apk/app-release.apk`

---

## 📜 Architectural Decisions (ADRs)

Key architectural choices are formally documented in [`docs/adr/`](docs/adr/):

- [ADR 0001: Rust Core with Flutter UI](docs/adr/0001-rust-core-with-flutter-ui.md)
- [ADR 0002: Strict Tor-Only Network Transport](docs/adr/0002-strict-tor-only-transport.md)
- [ADR 0003: Single-Device Profile Model](docs/adr/0003-single-device-profile-model.md)
- [ADR 0004: Argon2id Key Derivation with Biometric Caching](docs/adr/0004-argon2id-key-derivation-with-biometrics.md)
- [ADR 0005: SimpleX Queue HTTPS Port 443 Transport](docs/adr/0005-simplex-queue-https-port-443-transport.md)
- [ADR 0006: Compact Optical QR with Over-the-Wire Kyber Prekey](docs/adr/0006-compact-optical-qr-with-over-the-wire-kpk.md)
- [ADR 0007: Privacy-Scrubbed In-Memory Diagnostic Logger](docs/adr/0007-privacy-scrubbed-in-app-diagnostic-logger.md)
- [ADR 0008: C-ABI Dynamic Library FFI Bridge](docs/adr/0008-c-abi-dynamic-library-ffi-bridge.md)
- [ADR 0009: Conditional Bridge Factory for Web PWA](docs/adr/0009-conditional-compilation-for-web-pwa.md)
- [ADR 0010: Automated Multi-Arch Android NDK CI/CD Pipeline](docs/adr/0010-automated-ndk-ci-cd-release-pipeline.md)

---

## 🛡️ Security & Cryptographic Verifications

| Test Suite | Purpose | Status |
| :--- | :--- | :--- |
| `pqxdh_handshake_test` | FIPS 203 ML-KEM-1024 + X25519 root key derivation | ✅ Verified |
| `double_ratchet_test` | Out-of-order delivery, AEAD replay protection | ✅ Verified |
| `database_lifecycle_test` | Argon2id 256-bit AES master key unlock & invalid PIN rejection | ✅ Verified |
| `cascading_delete_test` | Atomic deletion of contact sessions & ciphertexts | ✅ Verified |
| `ffi_layer_tests` | C-ABI string allocation, error JSON encoding, queue exports | ✅ Verified |
| `live_chat_exchange_test` | Full two-party wire exchange over unidirectional queues | ✅ Verified |
| `relay_client_test` | Live TLS Port 443 push and poll roundtrip | ✅ Verified |
| `log_scrubber_test` | Automatic regex redaction of PINs and secret keys | ✅ Verified |

---

## 📄 License

AnonTrace is released under the **MIT License**. See [LICENSE](LICENSE) for details.
