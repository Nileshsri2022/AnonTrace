# Strict Tor-Only Network Transport

We decided to enforce 100% Tor-only strict network routing, connecting exclusively to Tor v3 `.onion` relay endpoints via an embedded in-process Arti client with zero clearnet fallback. While clearnet fallback provides availability when Tor circuits fail, it leaks the user's public IP address to relay operators; strict Tor routing guarantees non-negotiable anonymity and metadata elimination at the cost of transient offline state when Tor consensus is unreachable.
