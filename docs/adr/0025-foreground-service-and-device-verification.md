# ADR-0025: Foreground Service and Device Verification (Step 4)

## Status
Accepted (Code implemented locally 2026-10-24 — Issue 30; device
verification NOT done, no Android device in this VM)

## Context
Issue 26's 60 s background poll timer is frozen by Android Doze when
the app is backgrounded, and it does nothing when the app is killed.
`flutter_foreground_task` was already a dependency and the manifest
already declared the foreground service, but nothing wired it up.

## Decisions

1. **Foreground service, opt-in, tick-only background isolate.**
   The background isolate's only job is a 30 s tick to the main
   isolate. It does not poll the relay, decrypt, or re-init the Rust
   core, because it has no PIN and storing one for it would create a
   new secret-storage problem worse than the delivery problem it
   solves. The main isolate polls with the already-unlocked bridge.

2. **Service is user-started, never silent.** Battery cost is real,
   so the AppBar button starts/stops it and it stops on dispose.
   Starting it silently on unlock would trade battery for delivery
   without consent.

3. **No unkillable claim.** Even a foreground service can be killed
   by aggressive OEM battery savers. The device checklist in Issue
   30 §3 exists to measure that honestly per carrier/device, and a
   failed killed-case result must be reported with a battery-
   optimisation guide, not papered over.

4. **No FCM.** Adding Google push would give a wakeup when the
   service dies, at the cost of a Google dependency and metadata
   leakage to Google. That tradeoff was already rejected for the
   default path in ADR-0022; it stays rejected here.

## Consequences

- While the service runs, messages can arrive backgrounded; while
  it is stopped or killed, delivery waits for reopen.
- Battery use rises with the service on — measure it (Issue 30 §3
  row 8) before recommending it as default.
- This ADR + Issue 30 do not certify any device. Until the checklist
  is run on real phones, Step 4 is "wired, unverified" — exactly
  what its status line says.
