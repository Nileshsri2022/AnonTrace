use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use fips203::ml_kem_1024;
use fips203::traits::{Decaps, Encaps, KeyGen, SerDes};
use hkdf::Hkdf;
use rand::rngs::OsRng;
use sha2::Sha256;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

use crate::crypto::padding::CryptoError;

pub struct IdentityKeyPair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    pub dh_secret: StaticSecret,
    pub dh_public: XPublicKey,
}

pub fn generate_identity_keypair() -> IdentityKeyPair {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();

    let dh_secret = StaticSecret::random_from_rng(&mut csprng);
    let dh_public = XPublicKey::from(&dh_secret);

    IdentityKeyPair {
        signing_key,
        verifying_key,
        dh_secret,
        dh_public,
    }
}

#[derive(Clone, Debug)]
pub struct PreKeyBundle {
    pub identity_key: [u8; 32],
    pub identity_dh_key: [u8; 32],
    pub signed_prekey_id: u32,
    pub signed_prekey: [u8; 32],
    pub signature: [u8; 64],
    pub one_time_prekey_id: u32,
    pub one_time_prekey: [u8; 32],
    pub kyber_prekey: Vec<u8>,
}

impl PreKeyBundle {
    pub fn verify_signature(&self) -> bool {
        let verifying_key = match VerifyingKey::from_bytes(&self.identity_key) {
            Ok(k) => k,
            Err(_) => return false,
        };

        let signature = Signature::from_bytes(&self.signature);
        verifying_key.verify(&self.signed_prekey, &signature).is_ok()
    }
}

pub struct PreKeySecretBundle {
    pub signed_prekey_secret: StaticSecret,
    pub one_time_prekey_secret: StaticSecret,
    pub kyber_secret: ml_kem_1024::DecapsKey,
}

pub fn generate_prekey_bundle(
    identity: &IdentityKeyPair,
    signed_prekey_id: u32,
    one_time_prekey_id: u32,
) -> (PreKeyBundle, PreKeySecretBundle) {
    let mut csprng = OsRng;

    // Signed PreKey
    let signed_prekey_secret = StaticSecret::random_from_rng(&mut csprng);
    let signed_prekey_public = XPublicKey::from(&signed_prekey_secret);
    let signature = identity.signing_key.sign(signed_prekey_public.as_bytes());

    // One-Time PreKey
    let one_time_prekey_secret = StaticSecret::random_from_rng(&mut csprng);
    let one_time_prekey_public = XPublicKey::from(&one_time_prekey_secret);

    // Post-Quantum Kyber-1024 PreKey
    let (ek, dk) = ml_kem_1024::KG::try_keygen().expect("Kyber-1024 keygen");

    let bundle = PreKeyBundle {
        identity_key: identity.verifying_key.to_bytes(),
        identity_dh_key: *identity.dh_public.as_bytes(),
        signed_prekey_id,
        signed_prekey: *signed_prekey_public.as_bytes(),
        signature: signature.to_bytes(),
        one_time_prekey_id,
        one_time_prekey: *one_time_prekey_public.as_bytes(),
        kyber_prekey: ek.into_bytes().to_vec(),
    };

    let secrets = PreKeySecretBundle {
        signed_prekey_secret,
        one_time_prekey_secret,
        kyber_secret: dk,
    };

    (bundle, secrets)
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PqxdhMessage {
    pub sender_identity_key: [u8; 32],
    pub sender_identity_dh_key: [u8; 32],
    pub sender_ephemeral_key: [u8; 32],
    pub kyber_ciphertext: Vec<u8>,
    pub one_time_prekey_id: u32,
}

pub fn initiate_pqxdh(
    sender_identity: &IdentityKeyPair,
    recipient_bundle: &PreKeyBundle,
) -> Result<(PqxdhMessage, [u8; 32]), CryptoError> {
    if !recipient_bundle.verify_signature() {
        return Err(CryptoError::KeyExchangeFailed(
            "Recipient signed prekey signature verification failed".to_string(),
        ));
    }

    let mut csprng = OsRng;
    let ephemeral_secret = StaticSecret::random_from_rng(&mut csprng);
    let ephemeral_public = XPublicKey::from(&ephemeral_secret);

    let alice_identity_dh = XPublicKey::from(recipient_bundle.identity_dh_key);
    let alice_signed_prekey = XPublicKey::from(recipient_bundle.signed_prekey);
    let alice_one_time_prekey = XPublicKey::from(recipient_bundle.one_time_prekey);

    // DH1 = X25519(E_b, A_identity_dh)
    let dh1 = ephemeral_secret.diffie_hellman(&alice_identity_dh);
    // DH2 = X25519(B_identity_dh, A_signed_prekey)
    let dh2 = sender_identity.dh_secret.diffie_hellman(&alice_signed_prekey);
    // DH3 = X25519(E_b, A_signed_prekey)
    let dh3 = ephemeral_secret.diffie_hellman(&alice_signed_prekey);
    // DH4 = X25519(E_b, A_onetime_prekey)
    let dh4 = ephemeral_secret.diffie_hellman(&alice_one_time_prekey);

    // ML-KEM-1024 Encapsulation
    let ek = ml_kem_1024::EncapsKey::try_from_bytes(
        recipient_bundle.kyber_prekey.as_slice().try_into().map_err(|_| {
            CryptoError::KeyExchangeFailed("Invalid Kyber public key length".to_string())
        })?,
    )
    .map_err(|e| CryptoError::KeyExchangeFailed(format!("Invalid Kyber key: {:?}", e)))?;

    let (ss_pq, ct) = ek
        .try_encaps()
        .map_err(|e| CryptoError::KeyExchangeFailed(format!("Kyber encaps error: {:?}", e)))?;

    // Combine DH1 || DH2 || DH3 || DH4 || SS_PQ with HKDF-SHA256
    let mut ikm = Vec::with_capacity(32 * 4 + 32);
    ikm.extend_from_slice(dh1.as_bytes());
    ikm.extend_from_slice(dh2.as_bytes());
    ikm.extend_from_slice(dh3.as_bytes());
    ikm.extend_from_slice(dh4.as_bytes());
    ikm.extend_from_slice(&ss_pq.into_bytes());

    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut master_key = [0u8; 32];
    hk.expand(b"PQXDH-Signal-Simple-v1", &mut master_key)
        .map_err(|e| CryptoError::KeyExchangeFailed(e.to_string()))?;

    let message = PqxdhMessage {
        sender_identity_key: sender_identity.verifying_key.to_bytes(),
        sender_identity_dh_key: *sender_identity.dh_public.as_bytes(),
        sender_ephemeral_key: *ephemeral_public.as_bytes(),
        kyber_ciphertext: ct.into_bytes().to_vec(),
        one_time_prekey_id: recipient_bundle.one_time_prekey_id,
    };

    Ok((message, master_key))
}

pub fn process_pqxdh_initiation(
    recipient_identity: &IdentityKeyPair,
    recipient_secrets: &PreKeySecretBundle,
    message: &PqxdhMessage,
) -> Result<[u8; 32], CryptoError> {
    let bob_identity_dh = XPublicKey::from(message.sender_identity_dh_key);
    let bob_ephemeral = XPublicKey::from(message.sender_ephemeral_key);

    // DH1 = X25519(A_identity_dh, E_b)
    let dh1 = recipient_identity.dh_secret.diffie_hellman(&bob_ephemeral);
    // DH2 = X25519(A_signed_prekey, B_identity_dh)
    let dh2 = recipient_secrets.signed_prekey_secret.diffie_hellman(&bob_identity_dh);
    // DH3 = X25519(A_signed_prekey, E_b)
    let dh3 = recipient_secrets.signed_prekey_secret.diffie_hellman(&bob_ephemeral);
    // DH4 = X25519(A_onetime_prekey, E_b)
    let dh4 = recipient_secrets.one_time_prekey_secret.diffie_hellman(&bob_ephemeral);

    // ML-KEM-1024 Decapsulation
    let ct = ml_kem_1024::CipherText::try_from_bytes(
        message.kyber_ciphertext.as_slice().try_into().map_err(|_| {
            CryptoError::KeyExchangeFailed("Invalid Kyber ciphertext length".to_string())
        })?,
    )
    .map_err(|e| CryptoError::KeyExchangeFailed(format!("Invalid ciphertext: {:?}", e)))?;

    let ss_pq = recipient_secrets
        .kyber_secret
        .try_decaps(&ct)
        .map_err(|e| CryptoError::KeyExchangeFailed(format!("Kyber decaps error: {:?}", e)))?;

    // Combine matching DH1 || DH2 || DH3 || DH4 || SS_PQ with HKDF-SHA256
    let mut ikm = Vec::with_capacity(32 * 4 + 32);
    ikm.extend_from_slice(dh1.as_bytes());
    ikm.extend_from_slice(dh2.as_bytes());
    ikm.extend_from_slice(dh3.as_bytes());
    ikm.extend_from_slice(dh4.as_bytes());
    ikm.extend_from_slice(&ss_pq.into_bytes());

    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut master_key = [0u8; 32];
    hk.expand(b"PQXDH-Signal-Simple-v1", &mut master_key)
        .map_err(|e| CryptoError::KeyExchangeFailed(e.to_string()))?;

    Ok(master_key)
}
