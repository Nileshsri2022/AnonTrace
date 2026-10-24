# 23. Two-Job Parallel CI/CD Pipeline Optimization

| Property | Value |
| :--- | :--- |
| **Status** | Completed |
| **Type** | Infrastructure / Performance |
| **Release Target** | v1.0.14+ |
| **Blockers** | None |

---

## 1. Context & Motivation
Sequential CI/CD execution resulted in ~11-minute workflow runs on GitHub Actions due to redundant compilation of host debug tests and Android release binaries on a single machine.

---

## 2. Technical Scope
1. Update `.github/workflows/build-apk.yml`:
   - Introduce `verify` job running `cargo test --locked` and `flutter test`.
   - Streamline `build` job to cross-compile Android release binaries and package release APK with `--build-cache`.
   - Update `release` job to require `needs: [verify, build]`.
   - Add `CARGO_INCREMENTAL: "0"` and `RUSTFLAGS: "-C debuginfo=0"`.
2. Document in `docs/adr/0019-two-job-parallel-ci-cd-pipeline.md`.

---

## 3. Verification Criteria
- [x] `.github/workflows/build-apk.yml` syntax validated.
- [x] ADR-0019 committed.
- [x] Push to main.
