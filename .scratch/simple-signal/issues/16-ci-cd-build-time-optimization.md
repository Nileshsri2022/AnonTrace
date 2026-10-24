# 16. CI/CD Build Time Optimization & Thin LTO Pipeline

| Property | Value |
| :--- | :--- |
| **Status** | In Progress |
| **Type** | Infrastructure / Performance |
| **Release Target** | v1.0.12 |
| **Blockers** | 10, 15 |

---

## 1. Context & Motivation
On standard GitHub Actions 2-vCPU runners, building the Android release APK required over 11 minutes. Over 7.5 minutes of this time was consumed sequentially cross-compiling the Rust native core for 3 CPU architectures (`arm64-v8a`, `armeabi-v7a`, `x86_64`) with default single-threaded LTO.

---

## 2. Technical Scope
1. **Rust Release Profile (`crates/simple-signal-core/Cargo.toml`)**:
   - `opt-level = 3`
   - `lto = "thin"`
   - `codegen-units = 16`
   - `panic = "abort"`
   - `strip = true`
2. **Workflow Streamlining (`.github/workflows/build-apk.yml`)**:
   - Cache `cargo-ndk` binary under `~/.cargo/bin/cargo-ndk`.
   - Restrict release target architectures to consumer Android hardware (`arm64-v8a`, `armeabi-v7a`), removing desktop emulator `x86_64`.
   - Validate that local tests (`cargo test --locked`, `flutter test`) continue to pass.

---

## 3. Verification Criteria
- [x] `crates/simple-signal-core/Cargo.toml` specifies thin LTO and 16 codegen units.
- [x] `.github/workflows/build-apk.yml` caches `cargo-ndk` and targets real mobile architectures.
- [x] Native compilation builds cleanly without linkage errors.
