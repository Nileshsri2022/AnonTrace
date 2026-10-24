# 30. Real-World 4G & Battery Verification (Production Roadmap Step 4)

| Property | Value |
| :--- | :--- |
| **Status** | Code implemented locally 2026-10-24 (foreground service wiring); DEVICE VERIFICATION NOT DONE — no Android device in this VM |
| **Type** | Foreground service wiring + device test checklist |
| **Target** | `apps/simple_signal_mobile/` only (Flutter/Android, no Rust change) |
| **Related ADR** | `docs/adr/0025-foreground-service-and-device-verification.md` |
| **Out of scope** | Relay deployment (Issue 28), Flutter analyze (Issue 29), Snowflake/WebTunnel, FCM, groups/media/backup |

---

## 1. What changed in code

- New `lib/src/background/foreground_task.dart`: `flutter_foreground_task`
  6.5.0 wiring. Background isolate sends a 30 s tick to the main
  isolate; it never touches the relay, the Rust core, or any PIN
  (it has none — see ADR-0025 §2 for why that limit is deliberate).
- `lib/main.dart`: task-data callback polls the relay via the
  already-unlocked bridge on each tick; AppBar button toggles the
  service (opt-in, never silent on unlock); service stops on dispose.
- AndroidManifest already had the foreground-service permissions and
  the `ForegroundService` declaration — no manifest change was needed.

## 2. What this does NOT prove

Nothing here was run on a real phone. The following are claims to
verify, not results:

- That the service survives Doze, battery saver, and app-kill on
  Jio / Airtel / Vi, on CGNAT and IPv6-only networks.
- That a 30 s heartbeat's battery cost is acceptable (measure with
  Android battery stats, not by feel).
- That messages actually arrive while the screen is locked, and
  while the app is killed (the killed case may still fail on
  aggressive OEM skins — that outcome must be recorded, not hidden).

## 3. Device test checklist (operator must run, with a second phone)

For each carrier (Jio, Airtel, Vi), run every row and record
pass/fail + timestamps, not just "it worked":

1. **Foreground, screen on:** A → B message, measure delivery time.
2. **Backgrounded, screen on:** app in background 5 min, A → B.
3. **Screen locked 10 min:** A → B, check arrival time + whether a
   notification/tick woke the poll.
4. **App killed (swiped away):** A → B, check if the foreground
   service kept the process alive or delivery waited for reopen.
5. **Doze / battery saver on:** repeat rows 3–4 with battery saver on.
6. **Airplane toggle:** off → on → off, check SSE reconnect + poll
   catch-up, no duplicate messages.
7. **IPv6-only (if available):** repeat row 1.
8. **Battery:** note % drain over 1 hour with the service on vs off.

## 4. Known limits to record honestly

- Aggressive OEM battery savers (Xiaomi, Oppo, Vivo, Samsung with
  "sleeping apps") can kill even a foreground service. If row 4
  fails on such a device, the honest fix is a user-facing
  "disable battery optimisation" guide, not a claim that the service
  is unkillable.
- The 60 s background poll timer from Issue 26 is still present;
  it helps while backgrounded, not while killed. The foreground
  service is the killed-case attempt, not a guarantee.
- No FCM is involved. If the service is killed, there is no push
  wakeup — that is a deliberate privacy choice (no Google push
  dependency), and its cost is delayed delivery until reopen.
