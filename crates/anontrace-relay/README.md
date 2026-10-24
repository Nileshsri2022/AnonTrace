# anontrace-relay

Private relay for AnonTrace Normal Mode (Option B, Issue 26, ADR-0022).

## What it stores

Only: blind queue id, sender-auth hash, ciphertext (hex, as sent),
message id, arrival time. No accounts, no plaintext, no raw queue id,
no queue secret, no contact graph. Messages are deleted on ACK; a
7-day TTL is the backstop for un-ACKed messages. Logs must never
contain blind ids, IPs, or payloads (this binary logs only its bind
address, nothing per-request).

## Run

```sh
cargo run -p anontrace-relay -- 8080
```

## Deploy (operator decision, not code)

This binary speaks plain HTTP on loopback. Put TLS on port 443 in
front of it with a reverse proxy (nginx/Caddy) and point clients at
that HTTPS URL. Optionally expose an onion address for Stealth Mode.
Do not expose the relay's plain HTTP port to the internet.

## API (v1)

- `POST /q/<blind_id>/register` + header `X-Sender-Auth: <hex>` — create a queue.
- `POST /q/<blind_id>` + header `X-Sender-Auth: <hex>`, body = payload hex — send, returns `{"id":"m1"}`.
- `GET /q/<blind_id>/json?since=<id|all>[&poll=1]` — NDJSON messages after `since`. Unknown queues return 200 empty (no existence leak).
- `POST /q/<blind_id>/ack/<msg_id>` — delete, returns `{"deleted":true|false}`.
- `GET /health` — `{"status":"ok"}`.
