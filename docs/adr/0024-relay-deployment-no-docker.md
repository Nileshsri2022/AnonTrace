# ADR-0024: Relay Deployment — No Docker, Binary + systemd + Caddy + Tor

## Status
Accepted (Deployment package implemented locally 2026-10-24 — Issue 28;
live deploy NOT done, no VPS/domain/SSH in this VM)

## Context
The relay (`crates/anontrace-relay`) is a single Rust binary listening
on `127.0.0.1:8080` over plain HTTP. It has no database, no external
service, and no runtime dependency beyond libc. Docker's job —
isolating dependencies and packaging an environment — has nothing to
isolate here, so requiring Docker would add a moving part for no
benefit and would hide the actual deployment behind a container
boundary that makes systemd logs, Tor, and TLS harder to inspect.

## Decisions

1. **Binary + systemd, not Docker.** The release binary is copied to
   `/opt/anontrace-relay/` and run by a systemd unit with
   `Restart=always`, an unprivileged user, and `PrivateTmp`. Systemd
   already provides restart-on-crash, restart-on-boot, and journal
   logs — the three things Docker is usually bought for here.

2. **Caddy in front for 443 + TLS.** Caddy reverse-proxies
   `relay.anontrace.app` to `127.0.0.1:8080` and manages Let's
   Encrypt certificates itself. No certbot run, no certificate file
   to rotate by hand. The relay's plain HTTP port is never exposed
   to the internet: it binds loopback only, and the firewall opens
   only 22/80/443.

3. **Tor hidden service on the same server.** The onion address is
   just another front door to the same `127.0.0.1:8080` listener
   (`HiddenServicePort 80 127.0.0.1:8080`). No second relay, no
   second server.

4. **Onion client wiring is a separate follow-up.** The Flutter
   Stealth toggle currently bootstraps with no onion address, and
   there is no UI to enter one. Deploying an onion service does not
   by itself make the app use it; a small client change must pass
   the deployed onion address into `startStealthBootstrap`. That
   change is recorded here, not silently assumed.

5. **No 100% uptime promise.** No free tier guarantees it, and paid
   SLAs are ~99.9%. The closest free option is Oracle Always Free
   (no SLA, idle resources may be reclaimed). The deploy README
   states this plainly instead of claiming an uptime the operator
   cannot buy for free.

## Consequences

- An operator with a Linux VPS + DNS can deploy with the package in
  `crates/anontrace-relay/deploy/` without installing Docker.
- Updates are: rebuild, copy binary, `systemctl restart` — seconds
  of downtime, during which in-memory undelivered messages are
  lost (see below).
- **In-memory loss on restart is NOT fixed by this ADR.** The relay
  store is a HashMap. Until disk persistence is added, every restart
  (update, crash, reboot) loses undelivered messages. An operator
  must know this before calling Step 2 "done" for production.
