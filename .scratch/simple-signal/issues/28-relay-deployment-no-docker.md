# 28. Relay Deployment — No-Docker Package (Production Roadmap Step 2)

| Property | Value |
| :--- | :--- |
| **Status** | Implemented locally 2026-10-24 — deployment package only, live deploy NOT done (no VPS/domain/SSH in this VM) |
| **Type** | Operations / deployment — enables Step 2, does not itself deploy |
| **Target** | `crates/anontrace-relay/deploy/` (new) + relay README pointer |
| **Related ADR** | `docs/adr/0024-relay-deployment-no-docker.md` |
| **Out of scope** | Live VPS provisioning, DNS ownership, Tor onion client wiring in the app, relay disk persistence (see §5) |

---

## 1. What this is

The relay is a single Rust binary that listens on `127.0.0.1:8080`
and speaks plain HTTP. It needs no Docker: a binary + systemd +
Caddy (TLS) + Tor (onion) is the whole deployment. This issue adds
that package to the repo so an operator can deploy it in minutes
instead of reverse-engineering it from the code.

## 2. Files added

- `crates/anontrace-relay/deploy/README.md` — full operator guide:
  server prep, DNS, build, install, Tor onion, verification, updates.
- `crates/anontrace-relay/deploy/anontrace-relay.service` — systemd
  unit (restart always, unprivileged user, private tmp).
- `crates/anontrace-relay/deploy/Caddyfile` — `relay.anontrace.app`
  reverse-proxy to `127.0.0.1:8080`, Caddy manages Let's Encrypt.
- `crates/anontrace-relay/deploy/torrc` — hidden-service snippet
  (port 80 → `127.0.0.1:8080`).
- `crates/anontrace-relay/deploy/deploy.sh` — builds the release
  binary, copies it + the unit/Caddyfile to `$DEPLOY_HOST` over SSH,
  and enables the services. It never runs by itself in CI; it is an
  operator script, and it requires `DEPLOY_HOST` + `DOMAIN` to be
  set explicitly.
- `crates/anontrace-relay/deploy/verify.sh` — health checks:
  loopback, public HTTPS, and (if set) onion.

## 3. What this does NOT do

- It does not provision a VPS, buy/control a domain, or open SSH
  access. The operator must supply a server and set DNS
  (`relay.anontrace.app` → server IP) first.
- It does not deploy the relay live from this VM. There is no VPS
  here, and no credentials were supplied.
- It does not wire the onion address into the Flutter app. The
  Stealth toggle currently bootstraps with no onion; feeding the
  deployed onion address into the app is a separate small client
  change (recorded in ADR-0024 §4).
- It does not add disk persistence to the relay. The store is still
  in-memory: a restart loses undelivered messages. That is a relay
  code follow-up, not a deployment-package fix, and it is stated in
  the deploy README so an operator does not mistake "server up" for
  "messages safe".

## 4. Verification (local, no VPS)

- `bash -n deploy/deploy.sh && bash -n deploy/verify.sh` — syntax ok.
- `systemd-analyze verify deploy/anontrace-relay.service` — run by
  the operator on the target (not run here; no systemd target in
  this VM is authoritative for the server's systemd version).
- The relay binary itself is unchanged; `cargo test --locked` is
  unaffected (no Rust code touched).

## 5. Operator checklist before calling Step 2 done

1. Server + DNS + firewall (22/80/443 only).
2. `./deploy.sh` with `DEPLOY_HOST` + `DOMAIN`.
3. `./verify.sh` green (loopback + public HTTPS).
4. Tor onion hostname recorded, and the client onion-wiring
   follow-up filed.
5. Relay persistence follow-up decided (in-memory loss on restart).
