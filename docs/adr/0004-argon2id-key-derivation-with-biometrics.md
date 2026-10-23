# Argon2id Key Derivation with Biometric Caching

We decided to derive the SQLCipher 256-bit AES master database encryption key from an optional user PIN or Passphrase using Argon2id, with ephemeral session caching in the OS Keystore/Keyring guarded by biometrics. This prevents offline forensic database extraction if a physical device is seized while powered down or locked, while preserving instant biometrics-based unlock for daily mobile and desktop usability.
