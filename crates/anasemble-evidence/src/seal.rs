//! XChaCha20-Poly1305 payload seals. Same construction as the evidence plane.

use chacha20poly1305::aead::{Aead, Key, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};

use anasemble_core::{BoundedText, Digest, MAX_PAYLOAD_BYTES};

use crate::EvidenceError;

/// Sealed payload using `evidence-seal-v1` AAD.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SealedPayload {
    /// Seal version.
    pub version: String,
    /// Recovery key identifier.
    pub key_id: BoundedText,
    /// 24-byte nonce, hex-encoded.
    pub nonce_hex: String,
    /// Ciphertext, hex-encoded.
    pub ciphertext_hex: String,
    /// Digest of the plaintext payload.
    pub payload_digest: Digest,
}

/// Seal plaintext with XChaCha20-Poly1305.
pub fn seal_payload(
    plaintext: &[u8],
    key_id: &str,
    secret: &[u8; 32],
    nonce: &[u8; 24],
) -> Result<SealedPayload, EvidenceError> {
    if plaintext.is_empty() || plaintext.len() > MAX_PAYLOAD_BYTES {
        return Err(EvidenceError::Rejected("payload exceeds size limit"));
    }
    let key_id = BoundedText::new(key_id)?;
    let cipher = XChaCha20Poly1305::new(&Key::<XChaCha20Poly1305>::from(*secret));
    let aad = format!("evidence-seal-v1:{}", key_id.as_str());
    let ciphertext = cipher
        .encrypt(
            &XNonce::from(*nonce),
            Payload {
                msg: plaintext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| EvidenceError::Crypto("evidence encryption failed"))?;
    Ok(SealedPayload {
        version: "evidence-seal-v1".into(),
        key_id,
        nonce_hex: hex::encode(nonce),
        ciphertext_hex: hex::encode(ciphertext),
        payload_digest: Digest::sha256(plaintext),
    })
}

/// Open a sealed payload.
pub fn unseal_payload(sealed: &SealedPayload, secret: &[u8; 32]) -> Result<Vec<u8>, EvidenceError> {
    if sealed.version != "evidence-seal-v1" {
        return Err(EvidenceError::Rejected("evidence seal version is invalid"));
    }
    let nonce_bytes: [u8; 24] = hex::decode(&sealed.nonce_hex)
        .map_err(|_| EvidenceError::Crypto("nonce is not hex"))?
        .try_into()
        .map_err(|_| EvidenceError::Crypto("nonce is not 24 bytes"))?;
    let ciphertext = hex::decode(&sealed.ciphertext_hex)
        .map_err(|_| EvidenceError::Crypto("ciphertext is not hex"))?;
    if ciphertext.len() > MAX_PAYLOAD_BYTES {
        return Err(EvidenceError::Rejected("payload exceeds size limit"));
    }
    let cipher = XChaCha20Poly1305::new(&Key::<XChaCha20Poly1305>::from(*secret));
    let aad = format!("evidence-seal-v1:{}", sealed.key_id.as_str());
    let plaintext = cipher
        .decrypt(
            &XNonce::from(nonce_bytes),
            Payload {
                msg: &ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| EvidenceError::Crypto("evidence authentication failed"))?;
    if Digest::sha256(&plaintext) != sealed.payload_digest {
        return Err(EvidenceError::Rejected("payload digest mismatch"));
    }
    Ok(plaintext)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_round_trip() {
        let secret = [9_u8; 32];
        let nonce = [3_u8; 24];
        let sealed = seal_payload(b"hello", "rk1", &secret, &nonce).unwrap();
        assert_eq!(unseal_payload(&sealed, &secret).unwrap(), b"hello");
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let secret = [9_u8; 32];
        let nonce = [3_u8; 24];
        let mut sealed = seal_payload(b"hello", "rk1", &secret, &nonce).unwrap();
        sealed.ciphertext_hex.replace_range(0..2, "00");
        assert!(unseal_payload(&sealed, &secret).is_err());
    }
}
