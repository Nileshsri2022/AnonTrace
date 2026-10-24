# Deploying anontrace-relay WITHOUT Docker

The relay is a single Rust binary. No Docker, no database, no
external service. The whole deployment is: **binary + systemd +
Caddy (443/TLS) + Tor (onion, optional)** — all on one server.

## 0. Before you start (operator supplies these)

- A Linux VPS (Oracle Always Free works; see the uptime note below).
- DNS: an A record for your domain (e.g. `relay.anontrace.app`) →
  the server's public IP.
- SSH access to that server.
- Firewall: open 22 (SSH), 80, 443 only. **Do not open 8080.**
  The relay binds `127.0.0.1` only, so it is unreachable from the
  internet even if you forget — but do not rely on that alone.

## 1. Deploy

```sh
DEPLOY_HOST=user@server-ip DOMAIN=relay.anontrace.app ./deploy.sh
```

That builds the release binary, copies it to
`/opt/anontrace-relay/`, installs the systemd unit, and prints the
Caddy step. Then, on the server:

```sh
sudo apt install -y caddy
# /etc/caddy/Caddyfile — replace the domain with yours:
#   relay.anontrace.app {
#       reverse_proxy 127.0.0.1:8080
#   }
sudo systemctl reload caddy
```

Caddy obtains and renews the Let's Encrypt certificate itself.
No certbot, no certificate files to manage by hand.

## 2. Tor onion (optional, for Stealth Mode)

```sh
sudo apt install -y tor
# Append deploy/torrc to /etc/tor/torrc, then:
sudo systemctl restart tor
cat /var/lib/tor/anontrace-relay/hostname   # your .onion address
```

**Follow-up, not automatic:** the Flutter Stealth toggle currently
bootstraps with no onion address and there is no UI to enter one.
Once an onion is live, a small client change must pass that address
into `startStealthBootstrap`. Deploying the onion does not by itself
make the app use it.

## 3. Verify

```sh
# On the server:
curl http://127.0.0.1:8080/health   # {"status":"ok"}
# From anywhere:
DOMAIN=relay.anontrace.app ./verify.sh
```

## 4. Updates

Rebuild, copy the new binary, restart:

```sh
cargo build --release -p anontrace-relay
scp target/release/anontrace-relay "$DEPLOY_HOST:/opt/anontrace-relay/"
ssh "$DEPLOY_HOST" sudo systemctl restart anontrace-relay
```

Downtime is seconds — **but the relay store is in-memory, so a
restart loses undelivered messages.** Disk persistence is a separate
relay follow-up; do not call Step 2 "production done" until it is
decided.

## 5. Uptime honesty

No free tier guarantees 100% uptime, and paid SLAs are ~99.9%.
Free tiers that sleep (Render/Railway/HF) or auto-stop (Fly.io) are
unsuitable for a messenger relay. Oracle Always Free is the closest
free option and has **no SLA** — idle resources may be reclaimed.
If uptime must be guaranteed, use a paid VPS.

## 6. What this package does NOT do

- It does not provision a VPS, buy a domain, or set up SSH.
- It does not deploy live from a developer VM — there is no server
  here, and no credentials were supplied.
- It does not change the relay code, the API, or the app's default
  URL. The app still points at `https://relay.anontrace.app`; the
  operator must point that DNS name at the deployed server.
