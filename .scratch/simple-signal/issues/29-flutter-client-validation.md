# 29. Flutter Client Validation (Production Roadmap Step 3)

| Property | Value |
| :--- | :--- |
| **Status** | Partially implemented 2026-10-24 — PR checks workflow WRITTEN but NOT pushed (GitHub API 403, see §1b); LOCAL analyze NOT finished (see §3); device measurement NOT done |
| **Type** | Validation infrastructure + honest status record |
| **Target** | `.github/workflows/pr-checks.yml` (written locally, pending manual add — see §1b) |
| **Related ADR** | `docs/adr/0026-flutter-validation.md` |
| **Out of scope** | Relay deployment (Issue 28), device tests (Issue 30), release APK signing (Phase 0 follow-up) |

---

## 1. The gap this PR closes

`build-apk.yml` already runs Rust tests + Flutter tests, but only on
`v*` tags and manual dispatch. A PR could merge with broken Dart
code and nothing would check it. The new `pr-checks.yml` (written
in this branch's working tree) runs on every PR to `main`:
`cargo test --locked`, `flutter analyze`, and `flutter test`.
From now on, a PR with an analyze error fails its own checks
instead of being discovered at release time.

## 1b. Workflow file could NOT be pushed via the GitHub API

Pushing `.github/workflows/pr-checks.yml` through the connector
failed with `403 Resource not accessible by integration`, while
the docs in the same branch pushed fine. That is the GitHub App
lacking the `workflows` permission for this repo — workflow files
are a protected path via the API. **Operator step to complete
this PR:** add `.github/workflows/pr-checks.yml` manually (GitHub
web UI → Add file, or a local `git` push from a machine with
write access) using the file as written in the author's working
tree. Its content is short and is also quoted in the PR body.
Until that file lands, this PR is docs-only and the automatic
PR checks do not exist yet — do not mistake this PR for the
workflow itself.

## 2. What this PR does NOT claim

- It does not claim `flutter analyze` passed locally. See §3.
- It does not claim a release APK builds. That still needs Android
  SDK/NDK + a real `key.properties` (Phase 0 follow-up), and it is
  what `build-apk.yml` does on tags.
- It does not measure Argon2id unlock time. That needs a real
  low-end Android phone and is still an operator task.

## 3. Local analyze attempt (2026-10-24, honest record)

- No Flutter SDK existed in this VM, so one was downloaded
  (Flutter 3.35.7 stable, Linux tarball, ~1.4 GB) into
  `~/workspace/flutter-sdk/`.
- The download was proxy-slow: it reached ~50% after ~10 minutes
  and was still running when this PR was created, so waiting for
  it would have blocked the Step 2–4 PRs the user asked for.
- Consequence: this PR's Dart changes are NONE (only YAML + docs),
  so nothing in it can break `flutter analyze` — but the PRs it
  is meant to guard (Phase 0, Step 2, Step 4) still need their own
  analyze run before merging. The workflow in this PR will run
  that analyze automatically on each of those PRs once it merges
  — or the operator can run `flutter analyze` locally with the
  downloaded SDK once it finishes.
- If the local analyze, when it finishes, finds errors in the
  Phase 0 / Step 4 Dart changes, those must be fixed in follow-up
  commits on those PR branches, not silently merged.

## 4. Operator checklist for Step 3 completion

1. Merge this PR, then check that `pr-checks.yml` runs (green) on
   PRs #5 and #7.
2. Finish the local SDK download, run `flutter analyze` in
   `apps/simple_signal_mobile`, fix anything it reports.
3. On a real low-end Android phone, measure Argon2id (64 MiB)
   unlock time and record it here.
4. Run a release build with a real `key.properties` (after the
   Phase 0 key rotation) — that is the first true release-build
   proof, not this PR.
