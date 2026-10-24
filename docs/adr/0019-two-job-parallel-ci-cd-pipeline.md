# ADR-0019: Two-Job Parallel CI/CD Verification & Build Pipeline

## Status
Accepted

## Context
Previously, `.github/workflows/build-apk.yml` executed all tasks sequentially within a single `build` job:
1. Java 17 & Android NDK setup
2. Rust host + Android target toolchains setup
3. `cargo-ndk` installation
4. Host `cargo test --locked` (compiled debug binaries on host x86_64)
5. Android `cargo ndk` release compilation (compiled release binaries for arm64-v8a and armeabi-v7a)
6. Flutter setup & pub get
7. Host `flutter test`
8. `flutter build apk --release`

Because host test compilation (x86_64 debug) and Android cross-compilation (arm64/armv7 release) use completely distinct target directories and compiler profiles, compiling both on a single 2-vCPU runner caused redundant compilations of heavyweight cryptographic crates (Argon2, PQCrypto, OpenSSL, Tor), inflating pipeline duration to over 11 minutes.

## Decision
Refactor `.github/workflows/build-apk.yml` into a two-job parallel architecture:
1. **`verify` Job (Parallel Worker 1)**:
   - Dedicated solely to test execution (`cargo test --locked` + `flutter test`).
   - Omits Java, Android NDK, Android target toolchains, and `cargo-ndk`.
   - Runs on native host x86_64 and finishes in ~3 minutes.
2. **`build` Job (Parallel Worker 2)**:
   - Dedicated solely to Android APK artifact compilation.
   - Compiles release binaries directly for Android mobile targets (`arm64-v8a`, `armeabi-v7a`).
   - Uses `--build-cache` in Flutter Gradle build.
   - Global CI optimizations: `CARGO_INCREMENTAL: "0"` and `RUSTFLAGS: "-C debuginfo=0"`.
3. **`release` Job (Synchronized Release Publisher)**:
   - Declares `needs: [verify, build]`.
   - Publishes GitHub release assets only when both test verification and APK compilation succeed.

## Consequences
- **Positive**:
  - Total pipeline wall-clock time drops from ~11 minutes to ~4.5–5.5 minutes (~50% reduction).
  - Clean separation of concerns: test failures are immediately isolated to the `verify` job without wasting time on subsequent APK packaging.
  - Zero loss of verification: 100% of Rust core tests and Flutter widget tests must pass before any release is published.
- **Negative**:
  - Consumes two parallel GitHub Actions runner slots during the build window.
