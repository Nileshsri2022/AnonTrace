# ADR-0022: Dual Transport and Private Relay (Option B)

## Status
Accepted (Partially Implemented 2026-10-24 — Tickets 26.1 + 26.2 + 26.3 bootstrap/onion-post done, 26.3 remainder + 26.4 pending, see Issue 26 §8; merge together later, not pushed)

## Context
Option A (Issue 25, PR #2) fixed local security: identity and prekeys
persist, the database is SQLCipher-encrypted, and Argon2id uses a
random salt. The live transport still does not match the product's
metadata-privacy claim:

- The Flutter live path posts to public `https://ntfy.sh` via
  `RelayClient` (`bridge_ffi.dart`, `relay_client.dart`), with SSE
  and poll fallback, and ntfy retains messages.
- `net/tor.rs` never bootstraps an Arti circuit, Flutter's
  `HttpClient` has no SOCKS proxy, and the diagnostics screen
  hardcodes a Tor claim that is false on the live path.
- Relay auth is fake: `api.rs` uses `0x11`/`0x22` sender auth
  constants, and there is no per-inbox secret.
- On Indian 4G (Jio/Airtel/Vi, CGNAT, middleboxes, Android Doze),
  HTTPS on port 443 is the only transport that works on ~100% of
  phones; raw Tor/SOCKS and non-443 SMP do not.

This ADR records the decisions for Option B, before any code is
written, following the repo's ask-matt docs-first flow: issue 26 in
`.scratch/` is the working ticket, this ADR is the durable decision
record. It depends on Option A being merged first.

## Proposed Decisions & Architecture

1. **Dual transport, HTTPS default, Tor opt-in.**
   Normal Mode (default) is HTTPS 443 to a private relay, SSE +
   poll fallback. Stealth Mode (opt-in toggle) is embedded Arti in
   the Rust core, onion-first to the relay's onion address with
   Tor-exit fallback to the HTTPS relay. This supersedes ADR-0002
   (strict Tor-only) for the default path: Tor-only as the only
   transport made the app unreliable on Indian 4G. Tor remains,
   as the opt-in mode for users who need IP hiding.

2. **Private relay, ntfy-compatible read path + ACK delete.**
   A new Rust relay binary in this repo implements: `POST
   /q/<blind_id>` (sender-auth header), `GET
   /q/<blind_id>/json?since=` (SSE/NDJSON), `GET
   /q/<blind_id>/json?poll=1&since=` (one-shot poll), and `POST
   /q/<blind_id>/ack/<msg_id>` (delete on fetch/ACK, plus a TTL
   backstop). It stores only blind queue id, sender-auth hash,
   ciphertext, message id, and arrival time. No accounts, no
   plaintext, no retention logging.

3. **Blind queue id + real sender auth.**
   A per-inbox queue secret is generated on first init and persisted
   in the SQLCipher DB (Option A). `blind_id =
   HMAC-SHA256(queue_secret, "anontrace-inbound-v1")` is what the
   contact link carries, together with a real per-inbox sender auth
   key whose hash the relay stores at queue registration. The relay
   never sees the raw queue id or the queue secret, and a stranger
   who guesses a blind id cannot post without the sender auth key.
   Consequence: contact links change format once more, so
   pre-Option-B links must be re-shared. This is the second and
   last planned link break.

4. **Stealth in Rust, never Flutter SOCKS.**
   Stealth traffic goes through `arti-client` in the Rust core,
   bootstrapped with a Flutter-supplied data directory (never the
   current `./tor_data` default). Flutter `HttpClient` through a
   local SOCKS5 proxy or an external Orbot app is explicitly
   rejected as the Stealth path: it depends on a separate app and
   OS proxy behaviour that Doze and Indian carriers break. Arti
   bridge (Snowflake/WebTunnel) configuration is an open question
   for code time, to be recorded once resolved — not assumed here.

5. **Background: SSE + poll first, foreground task only as fallback.**
   Primary delivery is SSE with reconnect plus a 60 s poll fallback
   when locked. A foreground task with a 30 s heartbeat is the
   fallback for carriers that kill idle TCP in 60–120 s, with
   user-visible notification copy and a real-device battery
   measurement before it becomes default-on. FCM is not the
   default (Google metadata leakage, fails on de-Googled devices);
   an optional empty-payload FCM wakeup is a later opt-in ticket.

6. **Honest diagnostics and docs.**
   The diagnostics screen must show the mode actually in use
   (Normal + relay host, or Stealth via Arti), never a hardcoded
   Tor claim. README, SPEC, and ARCHITECTURE transport sections
   are updated in the same change.

## Consequences
- Normal Mode gives ~100% Indian 4G reach, at the honest cost that
  the (self-hosted, zero-retention) relay sees IP + blind id +
  timing/size in that mode. Users who need IP hiding use Stealth.
- Stealth Mode is slower and needs bridges on blocked ISPs, and
  must show bootstrap progress/failure honestly.
- No relay instance deployed means Normal Mode has nowhere to
  point: the app must fail with a clear "no relay configured"
  state, not silently fall back to ntfy.
- PQXDH, Double Ratchet, and SMP wire formats are untouched.
  Real SMP interop, FCM-as-default, UnifiedPush, groups, media,
  backup, and Keystore are out of scope for this issue.

## Verification
See issue 26 §5: full `cargo test --locked`, `flutter test`,
relay roundtrip + ACK-delete + auth-rejection tests, mode-switch
tests, diagnostics tests, grep gates (no ntfy default, no
`0x11`/`0x22` constants, no hardcoded Tor claim, no `./tor_data`
default), and a manual real-device checklist (Jio/Airtel/Vi,
IPv6-only, airplane toggle, Doze, app killed, app locked).
