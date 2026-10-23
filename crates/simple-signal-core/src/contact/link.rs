use std::collections::HashMap;
use thiserror::Error;

use crate::crypto::pqxdh::PreKeyBundle;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ContactLinkError {
    #[error("Invalid Contact Link URI scheme: {0}")]
    InvalidScheme(String),

    #[error("Missing required parameter in Contact Link: {0}")]
    MissingField(String),

    #[error("Malformed cryptographic key in Contact Link: {0}")]
    MalformedKey(String),

    #[error("Contact Link operation failed: {0}")]
    OperationFailed(String),
}

#[derive(Clone, Debug)]
pub struct ContactLink {
    pub relay_onion: String,
    pub receive_queue_id: String,
    pub sender_auth_key: Vec<u8>,
    pub prekey_bundle: PreKeyBundle,
}

impl ContactLink {
    /// Formats the ContactLink into a standard `simplex:/contact#/?...` URI string.
    pub fn to_uri(&self) -> Result<String, ContactLinkError> {
        let auth_hex = hex::encode(&self.sender_auth_key);
        let ik_hex = hex::encode(self.prekey_bundle.identity_key);
        let idhk_hex = hex::encode(self.prekey_bundle.identity_dh_key);
        let spk_hex = hex::encode(self.prekey_bundle.signed_prekey);
        let sig_hex = hex::encode(self.prekey_bundle.signature);
        let otpk_hex = hex::encode(self.prekey_bundle.one_time_prekey);
        let kpk_hex = hex::encode(&self.prekey_bundle.kyber_prekey);

        let mut query = format!(
            "v=1&relay={}&queue={}&auth={}&ik={}&idhk={}&spkid={}&spk={}&sig={}&otpkid={}&otpk={}",
            self.relay_onion,
            self.receive_queue_id,
            auth_hex,
            ik_hex,
            idhk_hex,
            self.prekey_bundle.signed_prekey_id,
            spk_hex,
            sig_hex,
            self.prekey_bundle.one_time_prekey_id,
            otpk_hex
        );

        if !self.prekey_bundle.kyber_prekey.is_empty() {
            query.push_str(&format!("&kpk={}", kpk_hex));
        }

        Ok(format!("simplex:/contact#/?{}", query))
    }

    /// Parses a standard `simplex:/contact#/?...` URI string into a ContactLink.
    pub fn from_uri(uri: &str) -> Result<Self, ContactLinkError> {
        const PREFIX: &str = "simplex:/contact#/?";
        if !uri.starts_with(PREFIX) {
            return Err(ContactLinkError::InvalidScheme(format!(
                "URI must begin with '{}'",
                PREFIX
            )));
        }

        let query_part = &uri[PREFIX.len()..];
        let mut params = HashMap::new();

        for pair in query_part.split('&') {
            let mut parts = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                params.insert(k, v);
            }
        }

        let relay_onion = params
            .get("relay")
            .ok_or_else(|| ContactLinkError::MissingField("relay".to_string()))?
            .to_string();

        let receive_queue_id = params
            .get("queue")
            .ok_or_else(|| ContactLinkError::MissingField("queue".to_string()))?
            .to_string();

        let auth_hex = params
            .get("auth")
            .ok_or_else(|| ContactLinkError::MissingField("auth".to_string()))?;
        let sender_auth_key = hex::decode(auth_hex)
            .map_err(|e| ContactLinkError::MalformedKey(format!("auth key hex: {}", e)))?;

        let ik_hex = params
            .get("ik")
            .ok_or_else(|| ContactLinkError::MissingField("ik".to_string()))?;
        let ik_vec = hex::decode(ik_hex)
            .map_err(|e| ContactLinkError::MalformedKey(format!("identity key hex: {}", e)))?;
        let identity_key: [u8; 32] = ik_vec
            .try_into()
            .map_err(|_| ContactLinkError::MalformedKey("identity key must be 32 bytes".to_string()))?;

        let idhk_hex = params
            .get("idhk")
            .ok_or_else(|| ContactLinkError::MissingField("idhk".to_string()))?;
        let idhk_vec = hex::decode(idhk_hex)
            .map_err(|e| ContactLinkError::MalformedKey(format!("identity dh key hex: {}", e)))?;
        let identity_dh_key: [u8; 32] = idhk_vec
            .try_into()
            .map_err(|_| ContactLinkError::MalformedKey("identity dh key must be 32 bytes".to_string()))?;

        let spkid: u32 = params
            .get("spkid")
            .ok_or_else(|| ContactLinkError::MissingField("spkid".to_string()))?
            .parse()
            .map_err(|_| ContactLinkError::MalformedKey("invalid spkid number".to_string()))?;

        let spk_hex = params
            .get("spk")
            .ok_or_else(|| ContactLinkError::MissingField("spk".to_string()))?;
        let spk_vec = hex::decode(spk_hex)
            .map_err(|e| ContactLinkError::MalformedKey(format!("signed prekey hex: {}", e)))?;
        let signed_prekey: [u8; 32] = spk_vec
            .try_into()
            .map_err(|_| ContactLinkError::MalformedKey("signed prekey must be 32 bytes".to_string()))?;

        let sig_hex = params
            .get("sig")
            .ok_or_else(|| ContactLinkError::MissingField("sig".to_string()))?;
        let sig_vec = hex::decode(sig_hex)
            .map_err(|e| ContactLinkError::MalformedKey(format!("signature hex: {}", e)))?;
        let signature: [u8; 64] = sig_vec
            .try_into()
            .map_err(|_| ContactLinkError::MalformedKey("signature must be 64 bytes".to_string()))?;

        let otpkid: u32 = params
            .get("otpkid")
            .ok_or_else(|| ContactLinkError::MissingField("otpkid".to_string()))?
            .parse()
            .map_err(|_| ContactLinkError::MalformedKey("invalid otpkid number".to_string()))?;

        let otpk_hex = params
            .get("otpk")
            .ok_or_else(|| ContactLinkError::MissingField("otpk".to_string()))?;
        let otpk_vec = hex::decode(otpk_hex)
            .map_err(|e| ContactLinkError::MalformedKey(format!("one-time prekey hex: {}", e)))?;
        let one_time_prekey: [u8; 32] = otpk_vec
            .try_into()
            .map_err(|_| ContactLinkError::MalformedKey("one-time prekey must be 32 bytes".to_string()))?;

        let kyber_prekey = if let Some(kpk_hex) = params.get("kpk") {
            hex::decode(kpk_hex)
                .map_err(|e| ContactLinkError::MalformedKey(format!("kyber prekey hex: {}", e)))?
        } else {
            Vec::new()
        };

        let prekey_bundle = PreKeyBundle {
            identity_key,
            identity_dh_key,
            signed_prekey_id: spkid,
            signed_prekey,
            signature,
            one_time_prekey_id: otpkid,
            one_time_prekey,
            kyber_prekey,
        };

        Ok(Self {
            relay_onion,
            receive_queue_id,
            sender_auth_key,
            prekey_bundle,
        })
    }
}
