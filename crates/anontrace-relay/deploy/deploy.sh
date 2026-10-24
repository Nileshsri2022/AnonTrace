#!/usr/bin/env bash
# Deploy the AnonTrace relay WITHOUT Docker: binary + systemd + Caddy.
# Operator script — it does NOT run by itself, and it needs a server.
#
# Usage:
#   DEPLOY_HOST=user@server-ip DOMAIN=relay.anontrace.app ./deploy.sh
#
# What it does, in order:
#   1. Builds the release binary locally (cargo build --release).
#   2. Copies the binary + systemd unit + Caddyfile to the server.
#   3. Creates the unprivileged user, installs the unit, enables it.
#   4. Installs Caddy if missing and reloads it with your domain.
#   5. Prints the verify command. Tor onion setup is manual (see
#      torrc in this directory) — it needs a tor restart + the
#      hostname file, which this script deliberately does not guess.
set -euo pipefail

: "${DEPLOY_HOST:?Set DEPLOY_HOST=user@server-ip}"
: "${DOMAIN:?Set DOMAIN=relay.anontrace.app (or your domain)}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

echo "==> Building release binary"
cargo build --release -p anontrace-relay --manifest-path "$REPO_ROOT/Cargo.toml"

echo "==> Copying to $DEPLOY_HOST"
ssh "$DEPLOY_HOST" 'sudo mkdir -p /opt/anontrace-relay && sudo chown "$USER" /opt/anontrace-relay || sudo mkdir -p /opt/anontrace-relay'
scp "$REPO_ROOT/target/release/anontrace-relay" "$DEPLOY_HOST:/opt/anontrace-relay/anontrace-relay"
scp "$SCRIPT_DIR/anontrace-relay.service" "$DEPLOY_HOST:/tmp/anontrace-relay.service"

echo "==> Installing service on $DEPLOY_HOST"
ssh "$DEPLOY_HOST" bash -s <<'EOS'
set -euo pipefail
id anontrace >/dev/null 2>&1 || sudo useradd --system --no-create-home anontrace
sudo cp /tmp/anontrace-relay.service /etc/systemd/system/anontrace-relay.service
sudo chown anontrace:anontrace /opt/anontrace-relay/anontrace-relay
sudo chmod 755 /opt/anontrace-relay/anontrace-relay
sudo systemctl daemon-reload
sudo systemctl enable --now anontrace-relay
sudo systemctl status anontrace-relay --no-pager
EOS

echo "==> Caddy (install if missing, then set domain)"
echo "On the server, run (once):"
echo "  sudo apt install -y caddy   # or: https://caddyserver.com/docs/install"
echo "  # Put this in /etc/caddy/Caddyfile (replace the domain):"
sed "s/relay.anontrace.app/$DOMAIN/" "$SCRIPT_DIR/Caddyfile"
echo "  sudo systemctl reload caddy"
echo ""
echo "==> Verify from your machine:"
echo "  DOMAIN=$DOMAIN ./verify.sh"
echo ""
echo "NOTE: the relay store is in-memory. A restart loses undelivered"
echo "messages. Disk persistence is a separate relay follow-up."
