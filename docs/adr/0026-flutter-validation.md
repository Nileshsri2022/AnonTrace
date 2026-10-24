# ADR-0026: Flutter Validation — PR Checks, Not Tag-Only Checks

## Status
Accepted (Workflow + Issue 29 implemented locally 2026-10-24;
local analyze NOT finished, see Issue 29 §3)

## Context
The repo already had a verify job (Rust tests + Flutter tests),
but it only ran on release tags and manual dispatch. That means
the exact moment a broken PR is cheapest to catch — before merge —
was the moment nothing checked it. Every PR in this session
(Option A/B, Phase 0, Step 2, Step 4) carried Dart changes that
were never compiled in the VM that wrote them.

## Decisions

1. **Every PR to `main` gets checked.** `pr-checks.yml` runs
   `cargo test --locked`, `flutter analyze`, and `flutter test`
   on `pull_request`. It is deliberately separate from
   `build-apk.yml` (which does the heavy NDK + APK build) so PR
   checks stay fast and do not need signing material.

2. **A workflow is not a substitute for a local run.** CI passing
   does not prove the operator's machine builds, and a local
   analyze that never ran is still "not run" even if CI is green
   later. Issue 29 §3 records the local attempt honestly instead
   of claiming a result that does not exist.

3. **Release builds stay tag-gated.** `build-apk.yml` is unchanged:
   a release APK still needs a tag, Android NDK, and a real
   `key.properties` after the Phase 0 key rotation. PR checks do
   not sign anything and must never be given signing secrets.

## Consequences

- After this merges, PRs #5 and #7 will get an automatic analyze
  run (on their next push, or on re-run) — that is the first real
  Dart compile check those PRs will have had.
- If that analyze finds errors, they are fixed on those PR
  branches, not by merging and hoping.
- Argon2id device timing and release-build proof remain operator
  tasks (Issue 29 §4); this ADR does not close them.
