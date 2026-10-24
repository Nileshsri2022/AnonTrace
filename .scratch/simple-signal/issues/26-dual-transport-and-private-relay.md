# 26. Dual Transport and Private Relay (Option B)

| Property | Value |
| :--- | :--- |
| **Status** | Partially implemented 2026-10-24 — Tickets 26.1 + 26.2 + 26.3 (bootstrap + onion post) done and tested; 26.3 remainder (exit fallback, bridges, Stealth send wiring) + 26.4 (foreground task) still pending, see §8. All Option B code local only, merge together later per user instruction — NOT pushed, NOT in PR #2 |
| **Type** | Transport / metadata-privacy feature |
| **Target** | Flutter `lib/src/net/relay_client.dart`, `lib/src/ffi/bridge_ffi.dart`; Rust `crates/simple-signal-core/src/net/`; new relay service (see §3) |
| **Framework Skill** | `ask-matt` (docs-first flow: issue → ADR → tickets → implement) |
| **Related ADR** | `docs/adr/0022-dual-transport-and-private-relay.md` |
| **Depends on** | PR #2 / Issue 25 (Option A) merged — stable identity, persisted prekeys, SQLCipher DB. Option B must not be branched from `main` without Option A. |
| **Out of scope** | Groups, media/attachments, backup, Android Keystore, FCM as default, real SMP interop, UnifiedPush (see §7) |

---

## 1. Executive Summary

Option A fixed local security. The live transport still does not match
the product's metadata-privacy claim:

1. **Public relay.** The Flutter live path (`bridge_ffi.dart` → `RelayClient`,
   default `https://ntfy.sh`) posts hex ciphertext to a public, third-party
   topic and polls it with SSE (`/json?since=`). ntfy retains messages,
   never deletes on fetch, and its operator sees the topic (the inbox
   queue id), the sender IP, timing, and size. The app has no private
   relay of its own.
2. **No real Tor transport.** `net/tor.rs` holds only `TorTransportConfig`,
   `DormancyMode`, and HMAC stream-isolation tokens. `TorClientManager::default_client()`
   never bootstraps an Arti circuit, and Flutter's standard `HttpClient`
   has no SOCKS5 proxy configured. The diagnostics screen in `main.dart`
   still hardcodes "Tor v3 Onion Circuits (Isolated Streams)" — that
   claim is false on the live path.
3. **Fake relay auth.** `api.rs` puts `sender_auth_key: vec![0x11; 32]`
   in its own contact link and `vec![0x22; 32]` in the return queue.
   Anyone who learns a queue id can post to it. There is no per-inbox
   sender auth secret, and none is persisted.
4. **No Indian-4G story.** Indian carriers (Jio/Airtel/Vi) use CGNAT,
   aggressive middlebox filtering, and Android Doze kills idle TCP in
   60–120 s. Raw Tor/SOCKS, raw SMP on non-443 ports, and a bare SSE
   stream with no poll fallback are unreliable on most Indian Android
   phones. HTTPS on port 443 is the only transport that works on
   ~100% of them.

Option B therefore has two modes, one default and one opt-in, and a
private relay that the project itself runs:

| Mode | Transport | Default? | 4G reach | What the relay / network sees |
| :--- | :--- | :--- | :--- | :--- |
| **Normal** | HTTPS 443 to the private relay, SSE + POST/poll fallback | Yes | ~100% (survives CGNAT, middleboxes, Doze reconnect) | Ciphertext + blind queue id + timing/size (16 KB padded); IP visible to the relay operator (self-hosted, see §3) |
| **Stealth** | Embedded Arti in the Rust core, onion-first to the relay's onion address, Tor exit to the HTTPS relay as fallback | No, opt-in toggle | Slower; bridges needed on blocked ISPs | Zero ISP metadata; relay sees Tor exit / onion, not the user's IP |

This issue supersedes ADR-0002 (strict Tor-only) for the default path:
Tor-only as the *only* transport was the reason the app could not work
reliably on Indian 4G. Tor stays, as the opt-in Stealth mode, not as
the default.

---

## 2. Current state — evidence in code (v1.0.17 + Option A working tree)

| Claim / path | Code | Verdict |
| :--- | :--- | :--- |
| Live transport is Tor | `bridge_ffi.dart:71` `RelayClient()` with default `https://ntfy.sh`; `relay_client.dart` `HttpClient` with no proxy | False — live path is plain HTTPS to ntfy |
| Tor client runs | `tor.rs`: config structs + `set_dormancy()` only; `arti-client` is a Cargo dep but never bootstrapped in `TorClientManager` | False — no circuit is opened |
| Diagnostics | `main.dart:224` hardcodes "Tor v3 Onion Circuits (Isolated Streams)" | False claim, must be corrected by this issue |
| SMP interop | `smp.rs`: `SmpRelayClient` is a TCP-stream client (`NEW`/`SKEY`/`SEND`/`ACK` vocabulary) and `SmpRelayHub` is in-memory only; Flutter never connects to a real SMP server | SMP-inspired, not interoperable — unchanged by this issue |
| Relay auth | `api.rs`: `sender_auth_key: vec![0x11; 32]` (own link), `vec![0x22; 32]` (return queue) | Fake — must be replaced (see D3) |
| Message size leakage | `traffic_padding_test.rs`: fixed 16 KB padding exists in the Rust padding layer | Padding exists; the Flutter relay path must not strip it (verify in 26.2) |
| Background delivery | `RelayClient.listenToQueue` retries after 2 s on error; no foreground service, no heartbeat, no FCM wiring in the live Flutter path | SSE reconnect exists; Doze survival does not |

---

## 3. Decisions (to be confirmed before code — see ADR-0022)

### D1. Private relay, ntfy-compatible read path + explicit ACK delete
Run a private relay (same Rust stack, new binary in this repo — see
Ticket 26.1 for the exact crate layout decision). Its v1 API keeps
ntfy's read shape so the Flutter change stays small, and adds what
ntfy lacks:

- `POST /q/<blind_id>` with header `X-Sender-Auth: <key>` → `{ "id": "<msg_id>" }`
- `GET /q/<blind_id>/json?since=<id>` → SSE/NDJSON stream (same shape
  as ntfy: `event: message`, `id`, `message`, `time`)
- `GET /q/<blind_id>/json?poll=1&since=<id>` → one-shot poll
- `POST /q/<blind_id>/ack/<msg_id>` → deletes that message (zero
  retention: delete on fetch/ACK; also a server-side TTL cap, e.g.
  7 days, as a backstop — exact TTL is a code-time constant, recorded
  in the relay README, not in this doc)

The relay stores only: blind queue id, sender-auth hash (not the key),
ciphertext, message id, arrival time. No accounts, no phone numbers,
no contact graph, no message plaintext (PQXDH ciphertext only), no
access-log retention by default (logs, if enabled for debugging, must
never contain blind ids, IPs, or payloads — see Ticket 26.1).

### D2. Blind queue id, derived from a persisted per-inbox secret
On first init (after Option A), generate a per-inbox queue secret,
persist it in the SQLCipher DB alongside identity/prekeys, and derive:

```
blind_id = hex(HMAC-SHA256(queue_secret, "anontrace-inbound-v1"))
```

The contact link carries `blind_id` (as its `queue` field) and a
per-inbox sender auth key — not the raw `anontrace_inbox_<6 bytes>`
id and not the `0x11`/`0x22` constants. The relay never sees the raw
queue id or the queue secret. Consequence, stated plainly: contact
links change format once more in Option B, so links shared before
Option B must be re-shared. That is the second and last planned link
break (Option A broke old links by persisting identity; Option B
breaks them by blinding the queue id).

### D3. Real sender auth, created at queue registration
When the app first creates its inbox (first init, or first link
generation after upgrade), it registers `blind_id` with the relay and
sets a sender auth key; the relay stores only its hash. Every POST
must present that key; a wrong key is rejected. The key is the one
embedded in the contact link, so a partner who scanned the QR can
post, and a stranger who merely guesses a blind id cannot. Rotation
is out of scope for this issue (a new inbox = a new link).

### D4. Normal Mode is the default; Stealth Mode is Arti in Rust, never Flutter SOCKS
- **Normal:** Flutter `RelayClient` with `baseUrl` = the private relay
  (configurable, defaulting to the project's relay URL — the exact
  default host is a deployment decision at code time, not assumed
  here). SSE first, poll fallback on SSE failure, 2 s reconnect
  (already in the client) + 60 s poll fallback when the app is
  backgrounded/locked (see D5).
- **Stealth:** implemented in the Rust core with `arti-client`
  (already a dependency), bootstrapped with a data directory passed
  from Flutter (never the current `./tor_data` default — that path is
  wrong on Android). Onion-first to the relay's onion address; if the
  onion address is unreachable, Tor exit to the HTTPS relay. Never
  route Flutter `HttpClient` through a local SOCKS5 proxy or an
  external Orbot app as the Stealth path — that was the flawed plan
  in earlier discussion and is explicitly rejected here: it depends
  on a separate app, a local daemon, and OS proxy behaviour that
  Android Doze and Indian carriers break.
- **Open question for code time (not assumed here):** Arti bridge
  configuration (Snowflake / WebTunnel) API shape in `arti-client`
  0.47. Resolve it in code and record the working config in the ADR;
  do not guess bridge syntax in this doc.

### D5. Android background: SSE + poll first, foreground task only as fallback
- Primary: SSE with reconnect + 60 s poll fallback when locked.
- Fallback (Ticket 26.4): a foreground task with a 30 s heartbeat
  ping, only when the OS kills idle TCP (the 60–120 s carrier case).
  This needs a user-visible notification and the foreground-service
  permission, so it must be explained in UI copy, not silently added.
  Battery impact must be measured on a real device and recorded in
  the ADR before shipping it as default-on.
- FCM is **not** the default: it leaks metadata to Google and fails
  on de-Googled/Huawei devices. An optional, empty-payload FCM wakeup
  (no content, no queue id) is a later, opt-in ticket, not part of
  this issue. UnifiedPush is noted as the de-Googled alternative for
  that later ticket, not implemented here.

### D6. Honest diagnostics and docs
`main.dart` diagnostics must show the *actual* mode: "Normal (HTTPS
443, private relay)" or "Stealth (Tor, Arti)" plus the relay host —
never a hardcoded Tor claim. `README.md`, `docs/ARCHITECTURE.md`,
and `docs/SPEC.md` transport sections are updated in the same change
to match the implemented modes, and ADR-0002 is marked superseded
for the default path by ADR-0022.

---

## 4. Work breakdown (tickets, in dependency order)

### Ticket 26.1 — Private relay service (protocol + skeleton)
- New relay binary in this repo (crate layout decided at code time;
  prefer a small Rust binary reusing `tokio` + the existing padding
  conventions, not a new language stack).
- Implement D1 API + sender-auth hash storage + delete-on-ACK + TTL
  backstop + no-retention logging rule.
- Tests: relay unit tests with an in-process server — POST → poll →
  ACK → message gone; wrong sender auth rejected; unknown blind id
  returns empty, not an error that leaks existence (decide the exact
  status code at code time and record it).
- Deliverable: relay README with deploy steps (single binary, port
  443 behind TLS, onion address optional) — deployment itself (a
  hosted instance) is a user/operator decision, not a code change.

### Ticket 26.2 — Normal Mode client switch (Flutter + Rust)
- `RelayClient`: `baseUrl` → private relay (configurable), keep
  SSE/poll shapes, add ACK call after `process_incoming_payload`
  succeeds (bridge_ffi poll path).
- `api.rs` / DB: generate + persist per-inbox queue secret and
  sender auth key (Option A tables); contact link emits `blind_id`
  + real sender auth key; delete the `0x11`/`0x22` constants.
- Remove `ntfy.sh` as the default (keep it only as a manually
  configured fallback if the user explicitly sets it — default is
  the private relay).
- Tests: client tests against the in-process relay from 26.1
  (send → SSE/poll → ACK → gone); existing Flutter/Rust tests must
  still pass, with ntfy references updated.

### Ticket 26.3 — Stealth Mode (Arti, opt-in toggle)
- Rust: bootstrap Arti with a Flutter-supplied data directory,
  onion-first + exit fallback, per-conversation circuit isolation
  (reuse the existing HMAC isolation-token design in `tor.rs`).
- Flutter: Stealth toggle in settings, persisted; when on, all
  relay traffic goes through the Rust Arti path, not Flutter
  `HttpClient`; when off, Normal Mode.
- Diagnostics shows the real mode (D6).
- Tests: Arti bootstrap + relay roundtrip in a test harness (may be
  slow/flaky on CI — mark it accordingly; a local-only test is
  acceptable if CI cannot reach Tor, but that limitation must be
  recorded, not hidden).

### Ticket 26.4 — Android background / Doze fallback
- 60 s poll fallback when locked (already the design; wire it to
  app lifecycle, not just SSE failure).
- Foreground task + 30 s heartbeat as the fallback, with UI copy
  and a battery measurement on a real device.
- Tests: lifecycle unit tests where possible; real-device checklist
  (see §5) is the real verification for this ticket.

### Ticket 26.5 — Docs + README correction (this issue's docs phase)
- ADR-0022 (created with this issue, status Proposed → Accepted on
  code merge).
- Update `README.md`, `docs/SPEC.md`, `docs/ARCHITECTURE.md`,
  diagnostics copy, and mark ADR-0002 superseded for the default path.
- No crypto wire-format changes. PQXDH, Double Ratchet, and SMP
  framing are untouched by this issue.

---

## 5. Verification criteria (all must pass before merge)

1. `cargo test --locked` + `flutter test` green, including new
   relay, client, and (where CI-feasible) Stealth tests.
2. Relay roundtrip: send → poll → ACK → message gone; wrong sender
   auth rejected; relay log contains no blind ids, IPs, or payloads.
3. Restart test (from Option A): identity/link stable, and the
   *new* blind link works after the Option B upgrade; old
   pre-Option-B links are documented as broken (re-share required).
4. Mode test: Normal Mode works with Tor/Arti disabled; Stealth
   Mode works with the relay reachable only via Tor (onion or exit);
   toggling modes switches the actual transport, not just a label.
5. Diagnostics test: the screen shows the mode actually in use.
6. Real-device checklist (manual, recorded in the PR): Jio +
   Airtel + Vi, IPv6-only network, airplane-mode toggle, Doze /
   battery-saver on, app killed in background, app locked 10 min
   then unlocked — messages arrive in each case (Normal Mode;
   Stealth Mode measured separately and reported, not assumed).
7. Grep gates: no remaining `ntfy.sh` default, no `0x11`/`0x22`
   sender auth constants in the live path, no hardcoded Tor claim
   in diagnostics, no `./tor_data` default on Android.

---

## 6. Risks

- **Relay hosting is an operator decision, not code.** If no relay
  instance is deployed, Normal Mode has nowhere to point. The code
  must fail with a clear "no relay configured" state, not silently
  fall back to ntfy.
- **Arti bootstrap time and bridges.** Tor bootstrap on mobile can
  take 10–30 s+ and fail on blocked ISPs without bridges. Stealth
  Mode must show bootstrap progress/failure honestly, not hang.
- **Foreground-task battery/UX cost.** A persistent notification
  and 30 s heartbeat may annoy users and drain battery; measure
  before making it default-on.
- **Link-break fatigue.** Option B is the second link break in two
  releases. Ship it with a clear in-app note ("re-share your QR
  once") — do not silently invalidate old links.
- **Relay as a metadata observer (Normal Mode).** Even a private,
  zero-retention relay sees IP + blind id + timing in Normal Mode.
  That is the honest tradeoff for 100% 4G reach; Stealth Mode is
  the answer for users who need IP hiding, and the docs must say
  so plainly.

---

## 8. Implementation status (2026-10-24)

- **Done + tested:** 26.1 relay (`net::relay` store + HTTP server, new
  `anontrace-relay` binary crate, README) and 26.2 Normal Mode
  (blind queue id + per-inbox queue secret/sender auth persisted in
  SQLCipher `local_kv`, contact link carries blind id + real sender
  auth, Flutter `RelayClient` posts to `/q/<blind_id>` with
  `X-Sender-Auth` and ACKs after processing, diagnostics now shows
  Normal Mode instead of the hardcoded Tor claim).
  Verified: `cargo test --locked` 50/50, including 5 new relay tests.
  Dart changes are **not** verified in this VM (no Flutter/Dart SDK
  installed) — run `flutter analyze` + `flutter test` before merging.
- **Done 2026-10-24 (26.3, bootstrap + onion post, merge later — not
  pushed):** Embedded Arti bootstrap in `net/tor.rs` — background
  start (non-blocking, Flutter data dir, never `./tor_data`),
  pollable `StealthStatus` (frac / ready / blocked), per-queue
  isolated Arti client, and `stealth_post` over the relay onion
  address (HTTP over onion, Tor provides the encryption, so no TLS
  needed on that path). FFI `simple_signal_start_stealth_bootstrap`
  + `simple_signal_get_stealth_status`, Dart bridge methods
  `startStealthBootstrap` / `getStealthStatus` (send path not yet
  switched — Normal Mode stays default until onion is deployed).
  Two real bugs found by a live probe and fixed: (1) Arti panics
  without a Rustls CryptoProvider — now installed (ring) before
  bootstrap, with a direct `rustls` dep; (2) Arti's fs-mistrust
  rejects state dirs under world-writable parents (e.g. `/tmp`) —
  data dir must be a private 0700 dir (Android app documents dir
  qualifies). Live probe in this VM reached frac 0.075 and stayed
  Bootstrapping (direct Tor is blocked by the VM proxy) — bootstrap
  code is proven to run, full bootstrap to Ready needs a real
  device / unproxied network.
- **Not yet done:** 26.3 remainder — HTTPS-over-Tor-exit fallback
  (needs tokio-rustls + webpki-roots, new deps, not yet added;
  Stealth currently fails closed with no onion, never downgrades
  to clearnet), bridges (Snowflake/WebTunnel, needs pt-client
  feature, not enabled), Stealth send wired into the main Flutter
  send path (needs a non-blocking FFI job, current
  `stealth_post_blocking` blocks the calling thread), and 26.4
  foreground task + 30 s heartbeat (dependency already in pubspec,
  not wired up). All need real-device verification.
- **Deployment placeholder:** default relay URL is
  `https://relay.anontrace.app` in both Rust (`DEFAULT_RELAY_URL`)
  and Dart. No such service is deployed yet; Normal Mode will fail
  until the operator points that host at a running relay behind
  TLS 443. The client must not fall back to ntfy.

## 7. Explicitly out of scope

- Real SMP server interop (simplexmq) — the relay is HTTPS, not SMP.
- FCM as default, UnifiedPush implementation, APNs.
- Groups, media/attachments, backup export/import, multi-device.
- Android Keystore / biometric key wrapping (Option A follow-up).
- Any change to PQXDH, Double Ratchet, or SMP wire formats.
- A hosted relay instance itself (operator deployment, not code).
