#!/usr/bin/env bash
# Health checks for a deployed relay. No arguments: checks loopback
# only if run ON the server. With DOMAIN set, checks public HTTPS.
# With ONION set, checks the onion address via tor (needs a local
# tor daemon or torsocks — skipped silently if absent).
set -u

fail=0

echo "==> Loopback (run on the server for a real check)"
curl -sf http://127.0.0.1:8080/health && echo "  loopback OK" || { echo "  loopback FAILED (expected if not on the server)"; fail=1; }

if [ -n "${DOMAIN:-}" ]; then
  echo "==> Public HTTPS: https://$DOMAIN/health"
  curl -sf "https://$DOMAIN/health" && echo "  public HTTPS OK" || { echo "  public HTTPS FAILED"; fail=1; }
else
  echo "==> Public HTTPS: skipped (DOMAIN not set)"
fi

if [ -n "${ONION:-}" ]; then
  echo "==> Onion: http://$ONION/health (via torsocks if available)"
  if command -v torsocks >/dev/null 2>&1; then
    torsocks curl -sf "http://$ONION/health" && echo "  onion OK" || { echo "  onion FAILED"; fail=1; }
  else
    echo "  torsocks not installed, onion check skipped"
  fi
else
  echo "==> Onion: skipped (ONION not set)"
fi

exit "$fail"
