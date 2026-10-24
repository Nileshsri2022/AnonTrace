# 10: Automated Multi-Arch APK Build & Signing Pipeline

**What to build:** A production GitHub Actions workflow (`.github/workflows/build-apk.yml`) that compiles the Rust core for `arm64-v8a`, `armeabi-v7a`, and `x86_64` using `cargo-ndk`, packages native libraries into the Flutter release bundle, signs the APK with v1 and v2 schemes, and publishes the artifact to GitHub Releases on tag pushes.

**Blocked by:** 07: Cross-Platform Flutter UI & FFI Bridge Integration.

**Status:** closed

- [x] Configures GitHub Actions CI runner with Java 17, Android NDK, Rust toolchain, and Flutter SDK.
- [x] Automates `cargo-ndk` multi-target compilation directly into `android/app/src/main/jniLibs`.
- [x] Configures Gradle release signing with permanent release keystore, enabling `v1SigningEnabled` and `v2SigningEnabled`.
- [x] Resolves Android 14+ `extractNativeLibs` and `targetSdkVersion 35` compatibility requirements.
- [x] Publishes release APK to GitHub Releases with dynamically extracted tag release notes on `v*` tag push.
