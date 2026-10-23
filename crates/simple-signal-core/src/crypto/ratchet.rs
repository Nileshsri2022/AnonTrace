use std::collections::HashMap;
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use sha2::Sha256;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

use crate::crypto::padding::CryptoError;

type HmacSha256 = Hmac<Sha256>;

pub struct DoubleRatchetSession {
    dhs: StaticSecret,
    dhs_pub: XPublicKey,
    dhr: Option<XPublicKey>,
    rk: [u8; 32],
    cks: Option<[u8; 32]>,
    ckr: Option<[u8; 32]>,
    ns: u32,
    nr: u32,
    pn: u32,
    mkskipped: HashMap<([u8; 32], u32), [u8; 32]>,
}

fn kdf_rk(rk: &[u8; 32], dh_out: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let hk = Hkdf::<Sha256>::new(Some(rk), dh_out);
    let mut okm = [0u8; 64];
    hk.expand(b"WhisperRatchetRoot", &mut okm)
        .expect("expand 64 bytes");
    let mut new_rk = [0u8; 32];
    let mut new_ck = [0u8; 32];
    new_rk.copy_from_slice(&okm[0..32]);
    new_ck.copy_from_slice(&okm[32..64]);
    (new_rk, new_ck)
}

fn kdf_ck(ck: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let mut mac1 = <HmacSha256 as Mac>::new_from_slice(ck).expect("hmac key");
    mac1.update(&[0x01]);
    let new_ck = mac1.finalize().into_bytes();

    let mut mac2 = <HmacSha256 as Mac>::new_from_slice(ck).expect("hmac key");
    mac2.update(&[0x02]);
    let mk = mac2.finalize().into_bytes();

    let mut out_ck = [0u8; 32];
    let mut out_mk = [0u8; 32];
    out_ck.copy_from_slice(&new_ck);
    out_mk.copy_from_slice(&mk);
    (out_ck, out_mk)
}

impl DoubleRatchetSession {
    /// Creates a new Double Ratchet session for the initiator (Alice), knowing Bob's initial DH public key
    pub fn new_initiator(shared_root_key: &[u8; 32], bob_dh_public: [u8; 32]) -> Self {
        let mut csprng = OsRng;
        let dhs = StaticSecret::random_from_rng(&mut csprng);
        let dhs_pub = XPublicKey::from(&dhs);

        let dhr = XPublicKey::from(bob_dh_public);
        let dh_out = dhs.diffie_hellman(&dhr);
        let (new_rk, cks) = kdf_rk(shared_root_key, dh_out.as_bytes());

        Self {
            dhs,
            dhs_pub,
            dhr: Some(dhr),
            rk: new_rk,
            cks: Some(cks),
            ckr: None,
            ns: 0,
            nr: 0,
            pn: 0,
            mkskipped: HashMap::new(),
        }
    }

    /// Creates a new Double Ratchet session for the receiver (Bob)
    pub fn new_receiver(shared_root_key: &[u8; 32], bob_dh_secret: StaticSecret) -> Self {
        let dhs_pub = XPublicKey::from(&bob_dh_secret);

        Self {
            dhs: bob_dh_secret,
            dhs_pub,
            dhr: None,
            rk: *shared_root_key,
            cks: None,
            ckr: None,
            ns: 0,
            nr: 0,
            pn: 0,
            mkskipped: HashMap::new(),
        }
    }

    pub fn current_dh_public(&self) -> [u8; 32] {
        *self.dhs_pub.as_bytes()
    }

    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let cks_val = self.cks.ok_or_else(|| {
            CryptoError::KeyExchangeFailed("Cannot encrypt before sending chain initialized".to_string())
        })?;

        let (new_cks, mk) = kdf_ck(&cks_val);
        self.cks = Some(new_cks);

        let n = self.ns;
        self.ns += 1;

        // Wire format: [ 32 bytes dhs_pub ] [ 4 bytes N ] [ 4 bytes PN ] [ 12 bytes Nonce ] [ Ciphertext ]
        let mut header = Vec::with_capacity(32 + 4 + 4);
        header.extend_from_slice(self.dhs_pub.as_bytes());
        header.extend_from_slice(&n.to_be_bytes());
        header.extend_from_slice(&self.pn.to_be_bytes());

        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[0..4].copy_from_slice(&n.to_be_bytes());
        nonce_bytes[4..12].copy_from_slice(&mk[0..8]);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let cipher = Aes256Gcm::new_from_slice(&mk)
            .map_err(|e| CryptoError::KeyExchangeFailed(e.to_string()))?;

        let ciphertext = cipher
            .encrypt(nonce, Payload { msg: plaintext, aad: &header })
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;

        let mut output = Vec::with_capacity(header.len() + 12 + ciphertext.len());
        output.extend_from_slice(&header);
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);

        Ok(output)
    }

    pub fn decrypt(&mut self, wire_bytes: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if wire_bytes.len() < 32 + 4 + 4 + 12 + 16 {
            return Err(CryptoError::DecryptionFailed("Ciphertext too short".to_string()));
        }

        let mut peer_pub_bytes = [0u8; 32];
        peer_pub_bytes.copy_from_slice(&wire_bytes[0..32]);
        let peer_pub = XPublicKey::from(peer_pub_bytes);

        let n = u32::from_be_bytes(wire_bytes[32..36].try_into().unwrap());
        let _pn = u32::from_be_bytes(wire_bytes[36..40].try_into().unwrap());
        let nonce_bytes = &wire_bytes[40..52];
        let ciphertext = &wire_bytes[52..];
        let header = &wire_bytes[0..40];

        // 1. Check if we already have a skipped message key for this (peer_pub, n)
        if let Some(mk) = self.mkskipped.remove(&(peer_pub_bytes, n)) {
            let cipher = Aes256Gcm::new_from_slice(&mk)
                .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;
            let nonce = Nonce::from_slice(nonce_bytes);
            return cipher
                .decrypt(nonce, Payload { msg: ciphertext, aad: header })
                .map_err(|e| CryptoError::DecryptionFailed(e.to_string()));
        }

        // 2. Check if a DH ratchet step is needed
        let need_dh_ratchet = match self.dhr {
            Some(existing_dhr) => existing_dhr.as_bytes() != peer_pub.as_bytes(),
            None => true,
        };

        if need_dh_ratchet {
            // Skip any remaining messages in the previous receiving chain
            if let Some(ckr_val) = self.ckr {
                self.skip_message_keys_until(self.nr, ckr_val);
            }

            // DH Ratchet step
            self.pn = self.ns;
            self.ns = 0;
            self.nr = 0;
            self.dhr = Some(peer_pub);

            let dh_out_recv = self.dhs.diffie_hellman(&peer_pub);
            let (new_rk, ckr) = kdf_rk(&self.rk, dh_out_recv.as_bytes());
            self.rk = new_rk;
            self.ckr = Some(ckr);

            // Advance sending chain for our next reply
            let mut csprng = OsRng;
            self.dhs = StaticSecret::random_from_rng(&mut csprng);
            self.dhs_pub = XPublicKey::from(&self.dhs);

            let dh_out_send = self.dhs.diffie_hellman(&peer_pub);
            let (new_rk_send, cks) = kdf_rk(&self.rk, dh_out_send.as_bytes());
            self.rk = new_rk_send;
            self.cks = Some(cks);
        }

        // 3. Skip message keys in the current chain if n > self.nr
        let mut current_ckr = self.ckr.ok_or_else(|| {
            CryptoError::DecryptionFailed("No receiving chain initialized".to_string())
        })?;

        while self.nr < n {
            let (next_ckr, mk) = kdf_ck(&current_ckr);
            self.mkskipped.insert((peer_pub_bytes, self.nr), mk);
            self.nr += 1;
            current_ckr = next_ckr;
        }

        // 4. Derive message key for message n
        let (next_ckr, mk) = kdf_ck(&current_ckr);
        self.ckr = Some(next_ckr);
        self.nr += 1;

        // 5. Decrypt payload
        let cipher = Aes256Gcm::new_from_slice(&mk)
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;
        let nonce = Nonce::from_slice(nonce_bytes);

        cipher
            .decrypt(nonce, Payload { msg: ciphertext, aad: header })
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))
    }

    fn skip_message_keys_until(&mut self, until: u32, mut ckr: [u8; 32]) {
        if let Some(peer_dhr) = self.dhr {
            let peer_bytes = *peer_dhr.as_bytes();
            while self.nr < until {
                let (next_ckr, mk) = kdf_ck(&ckr);
                self.mkskipped.insert((peer_bytes, self.nr), mk);
                self.nr += 1;
                ckr = next_ckr;
            }
        }
    }

    /// Serializes the ratchet session state into bytes for secure persistence in database
    pub fn to_bytes(&self) -> Result<Vec<u8>, CryptoError> {
        let snapshot = RatchetStateSnapshot {
            dhs_bytes: self.dhs.to_bytes(),
            dhs_pub: *self.dhs_pub.as_bytes(),
            dhr_pub: self.dhr.as_ref().map(|pk| *pk.as_bytes()),
            rk: self.rk,
            cks: self.cks,
            ckr: self.ckr,
            ns: self.ns,
            nr: self.nr,
            pn: self.pn,
            mkskipped: self
                .mkskipped
                .iter()
                .map(|(&(pk, n), &mk)| (pk, n, mk))
                .collect(),
        };

        serde_json::to_vec(&snapshot)
            .map_err(|e| CryptoError::KeyExchangeFailed(format!("Session serialize failed: {}", e)))
    }

    /// Deserializes a ratchet session from stored bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let snapshot: RatchetStateSnapshot = serde_json::from_slice(bytes)
            .map_err(|e| CryptoError::KeyExchangeFailed(format!("Session deserialize failed: {}", e)))?;

        let dhs = StaticSecret::from(snapshot.dhs_bytes);
        let dhs_pub = XPublicKey::from(snapshot.dhs_pub);
        let dhr = snapshot.dhr_pub.map(XPublicKey::from);

        let mkskipped = snapshot
            .mkskipped
            .into_iter()
            .map(|(pk, n, mk)| ((pk, n), mk))
            .collect();

        Ok(Self {
            dhs,
            dhs_pub,
            dhr,
            rk: snapshot.rk,
            cks: snapshot.cks,
            ckr: snapshot.ckr,
            ns: snapshot.ns,
            nr: snapshot.nr,
            pn: snapshot.pn,
            mkskipped,
        })
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RatchetStateSnapshot {
    dhs_bytes: [u8; 32],
    dhs_pub: [u8; 32],
    dhr_pub: Option<[u8; 32]>,
    rk: [u8; 32],
    cks: Option<[u8; 32]>,
    ckr: Option<[u8; 32]>,
    ns: u32,
    nr: u32,
    pn: u32,
    mkskipped: Vec<([u8; 32], u32, [u8; 32])>,
}
