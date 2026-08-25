//! Generic EvidenceEnvelopeV1.

use std::collections::BTreeMap;
use std::str::FromStr;

use anasemble_core::{
    BoundedText, Digest, MAX_PAYLOAD_BYTES, ObservationId, TenantId, encode_canonical_cbor,
};
use ed25519_dalek::{Signature, Signer, SigningKey as Ed25519SigningKey, Verifier, VerifyingKey};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::{EvidenceError, MAX_LABELS, MAX_MEDIA_TYPE_BYTES};

/// Envelope schema version.
pub const SCHEMA_VERSION: u16 = 1;

/// Evidence kind. Independent of [`TrustLevel`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// Declared intent.
    Declared,
    /// Observed runtime or catalog evidence.
    Observed,
    /// Executed change.
    Executed,
    /// State-bearing evidence.
    State,
    /// Contractual evidence.
    Contractual,
    /// Independently certified evidence.
    Verified,
    /// Deterministic derivation.
    Derived,
    /// Inference or hypothesis.
    Inferred,
    /// Explicit absence.
    Negative,
    /// Mutually incompatible evidence.
    Contradictory,
}

/// Trust is orthogonal to kind.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// No authentication succeeded.
    Unverified,
    /// Source authentication succeeded.
    Authenticated,
    /// Digest and signature verified.
    IntegrityVerified,
    /// Independently corroborated.
    IndependentlyCorroborated,
    /// Certified by a verifier certificate.
    Certified,
    /// Key or evidence revoked.
    Revoked,
}

/// Half-open valid-time interval. `end = None` means currently believed valid.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValidInterval {
    /// Inclusive start as RFC 3339 UTC.
    pub start: String,
    /// Exclusive end as RFC 3339 UTC, or omitted when open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
}

impl ValidInterval {
    /// Parse and reject empty intervals.
    pub fn validate(&self) -> Result<(), EvidenceError> {
        let start = parse_time(&self.start)?;
        if let Some(end) = &self.end {
            let end = parse_time(end)?;
            if end.as_microsecond() <= start.as_microsecond() {
                return Err(EvidenceError::Rejected("valid interval is empty"));
            }
        }
        Ok(())
    }
}

/// Provider identity. Display names are not stored here.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceIdentity {
    /// Provider kind, for example `github`.
    pub kind: BoundedText,
    /// Provider instance identity.
    pub instance: BoundedText,
}

/// Subject of the envelope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubjectRef {
    /// Ontology type.
    pub entity_type: BoundedText,
    /// Canonical external identity.
    pub canonical_external_identity: BoundedText,
}

/// Collection checkpoint context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CollectionContext {
    /// Adapter checkpoint token.
    pub checkpoint: BoundedText,
}

/// Payload handling class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadClass {
    /// Allowlisted metadata.
    Metadata,
    /// Digest-bound ciphertext.
    EncryptedObject,
    /// Redacted placeholder.
    Redacted,
    /// Must not be persisted.
    Forbidden,
}

/// Payload that may be plaintext metadata, sealed, or redacted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "class", rename_all = "snake_case")]
pub enum RedactablePayload {
    /// Allowlisted metadata bytes, hex-encoded.
    Metadata {
        /// Hex-encoded payload bytes.
        bytes_hex: String,
    },
    /// XChaCha20-Poly1305 sealed object.
    EncryptedObject {
        /// Sealing key identifier.
        key_id: BoundedText,
        /// 24-byte nonce, hex-encoded.
        nonce_hex: String,
        /// Ciphertext, hex-encoded.
        ciphertext_hex: String,
    },
    /// Redacted after classification.
    Redacted {
        /// Machine-readable reason.
        reason: BoundedText,
    },
}

impl RedactablePayload {
    /// Payload class used by admission.
    #[must_use]
    pub fn class(&self) -> PayloadClass {
        match self {
            Self::Metadata { .. } => PayloadClass::Metadata,
            Self::EncryptedObject { .. } => PayloadClass::EncryptedObject,
            Self::Redacted { .. } => PayloadClass::Redacted,
        }
    }
}

/// Detached signature block. Not included in the digest preimage.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignatureBlock {
    /// Signature algorithm, currently `ed25519`.
    pub algorithm: BoundedText,
    /// Key identifier.
    pub key_id: BoundedText,
    /// Hex-encoded signature bytes.
    pub value: String,
}

/// Generic evidence envelope version 1.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceEnvelopeV1 {
    /// Schema version. Must be 1.
    pub schema_version: u16,
    /// Observation identity.
    pub observation_id: ObservationId,
    /// Tenant bound into the envelope.
    pub tenant_id: TenantId,
    /// Source identity.
    pub source: SourceIdentity,
    /// Subject identity.
    pub subject: SubjectRef,
    /// Evidence kind.
    pub kind: EvidenceKind,
    /// Trust established after verification.
    pub trust: TrustLevel,
    /// Adapter observation time, RFC 3339 UTC.
    pub observed_at: String,
    /// Provider timestamp when supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_time: Option<String>,
    /// Valid-time interval.
    pub valid: ValidInterval,
    /// Receiver acceptance time, RFC 3339 UTC.
    pub received_at: String,
    /// Payload media type.
    pub payload_media_type: String,
    /// SHA-256 of payload bytes before sealing.
    pub payload_digest: Digest,
    /// Redactable payload.
    pub payload: RedactablePayload,
    /// Previous envelope digest when chaining.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predecessor: Option<Digest>,
    /// Collection context.
    pub collection: CollectionContext,
    /// Bounded labels.
    pub labels: BTreeMap<String, String>,
    /// Signature over canonical CBOR of the unsigned envelope.
    pub signature: SignatureBlock,
}

#[derive(Serialize)]
struct UnsignedEnvelope<'a> {
    schema_version: u16,
    observation_id: ObservationId,
    tenant_id: TenantId,
    source: &'a SourceIdentity,
    subject: &'a SubjectRef,
    kind: EvidenceKind,
    trust: TrustLevel,
    observed_at: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_time: Option<&'a str>,
    valid: &'a ValidInterval,
    received_at: &'a str,
    payload_media_type: &'a str,
    payload_digest: Digest,
    payload: &'a RedactablePayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    predecessor: Option<Digest>,
    collection: &'a CollectionContext,
    labels: &'a BTreeMap<String, String>,
}

impl EvidenceEnvelopeV1 {
    fn unsigned(&self) -> UnsignedEnvelope<'_> {
        UnsignedEnvelope {
            schema_version: self.schema_version,
            observation_id: self.observation_id,
            tenant_id: self.tenant_id,
            source: &self.source,
            subject: &self.subject,
            kind: self.kind,
            trust: self.trust,
            observed_at: &self.observed_at,
            source_time: self.source_time.as_deref(),
            valid: &self.valid,
            received_at: &self.received_at,
            payload_media_type: &self.payload_media_type,
            payload_digest: self.payload_digest,
            payload: &self.payload,
            predecessor: self.predecessor,
            collection: &self.collection,
            labels: &self.labels,
        }
    }

    /// Canonical CBOR of the unsigned envelope. This is the digest preimage.
    pub fn canonical_cbor(&self) -> Result<Vec<u8>, EvidenceError> {
        Ok(encode_canonical_cbor(&self.unsigned())?)
    }

    /// SHA-256 of [`Self::canonical_cbor`].
    pub fn digest(&self) -> Result<Digest, EvidenceError> {
        Ok(Digest::sha256(&self.canonical_cbor()?))
    }

    /// Sign with Ed25519. Replaces `signature`.
    pub fn sign(mut self, key_id: &str, secret: &[u8; 32]) -> Result<Self, EvidenceError> {
        let key_id = BoundedText::new(key_id)?;
        self.trust = TrustLevel::IntegrityVerified;
        self.signature = SignatureBlock {
            algorithm: BoundedText::new("ed25519")?,
            key_id: key_id.clone(),
            value: String::new(),
        };
        let preimage = self.canonical_cbor()?;
        let signature = Ed25519SigningKey::from_bytes(secret).sign(&preimage);
        self.signature.value = hex::encode(signature.to_bytes());
        Ok(self)
    }

    /// Verify the Ed25519 signature against `public`.
    pub fn verify_ed25519(&self, public: &[u8; 32]) -> Result<(), EvidenceError> {
        if self.signature.algorithm.as_str() != "ed25519" {
            return Err(EvidenceError::Crypto("unsupported signature algorithm"));
        }
        let bytes = hex::decode(&self.signature.value)
            .map_err(|_| EvidenceError::Crypto("signature is not hex"))?;
        let bytes: [u8; 64] = bytes
            .try_into()
            .map_err(|_| EvidenceError::Crypto("signature is not 64 bytes"))?;
        let verifying = VerifyingKey::from_bytes(public)
            .map_err(|_| EvidenceError::Crypto("public key is invalid"))?;
        let signature = Signature::from_bytes(&bytes);
        verifying
            .verify(&self.canonical_cbor()?, &signature)
            .map_err(|_| EvidenceError::Crypto("signature verification failed"))?;
        Ok(())
    }

    /// Field bounds that do not require a key.
    pub fn check_bounds(&self) -> Result<(), EvidenceError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(EvidenceError::Rejected("unsupported schema version"));
        }
        self.valid.validate()?;
        parse_time(&self.observed_at)?;
        parse_time(&self.received_at)?;
        if let Some(source_time) = &self.source_time {
            parse_time(source_time)?;
        }
        if self.payload_media_type.is_empty()
            || self.payload_media_type.len() > MAX_MEDIA_TYPE_BYTES
        {
            return Err(EvidenceError::Rejected(
                "media type is missing or oversized",
            ));
        }
        if self.labels.len() > MAX_LABELS
            || self
                .labels
                .iter()
                .any(|(k, v)| k.is_empty() || k.len() > 256 || v.len() > 256)
        {
            return Err(EvidenceError::Rejected("labels exceed policy"));
        }
        match &self.payload {
            RedactablePayload::Metadata { bytes_hex } => {
                let bytes = hex::decode(bytes_hex)
                    .map_err(|_| EvidenceError::Rejected("payload is not hex"))?;
                if bytes.len() > MAX_PAYLOAD_BYTES {
                    return Err(EvidenceError::Rejected("payload exceeds size limit"));
                }
                if Digest::sha256(&bytes) != self.payload_digest {
                    return Err(EvidenceError::Rejected("payload digest mismatch"));
                }
            }
            RedactablePayload::EncryptedObject { ciphertext_hex, .. } => {
                let bytes = hex::decode(ciphertext_hex)
                    .map_err(|_| EvidenceError::Rejected("ciphertext is not hex"))?;
                if bytes.len() > MAX_PAYLOAD_BYTES {
                    return Err(EvidenceError::Rejected("payload exceeds size limit"));
                }
            }
            RedactablePayload::Redacted { .. } => {}
        }
        Ok(())
    }
}

pub(crate) fn parse_time(value: &str) -> Result<Timestamp, EvidenceError> {
    Timestamp::from_str(value).map_err(|_| EvidenceError::Timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anasemble_core::BoundedText;

    fn sample() -> EvidenceEnvelopeV1 {
        let payload = b"{\"ok\":true}";
        EvidenceEnvelopeV1 {
            schema_version: 1,
            observation_id: ObservationId::generate(),
            tenant_id: TenantId::generate(),
            source: SourceIdentity {
                kind: BoundedText::new("github").unwrap(),
                instance: BoundedText::new("github.com").unwrap(),
            },
            subject: SubjectRef {
                entity_type: BoundedText::new("repository").unwrap(),
                canonical_external_identity: BoundedText::new("github.com:1").unwrap(),
            },
            kind: EvidenceKind::Observed,
            trust: TrustLevel::Unverified,
            observed_at: "2026-08-24T22:00:00.000000Z".into(),
            source_time: None,
            valid: ValidInterval {
                start: "2026-08-24T22:00:00.000000Z".into(),
                end: None,
            },
            received_at: "2026-08-24T22:00:01.000000Z".into(),
            payload_media_type: "application/json".into(),
            payload_digest: Digest::sha256(payload),
            payload: RedactablePayload::Metadata {
                bytes_hex: hex::encode(payload),
            },
            predecessor: None,
            collection: CollectionContext {
                checkpoint: BoundedText::new("0").unwrap(),
            },
            labels: BTreeMap::new(),
            signature: SignatureBlock {
                algorithm: BoundedText::new("ed25519").unwrap(),
                key_id: BoundedText::new("k1").unwrap(),
                value: String::new(),
            },
        }
    }

    #[test]
    fn sign_and_verify_round_trip() {
        let secret = [7_u8; 32];
        let public = Ed25519SigningKey::from_bytes(&secret)
            .verifying_key()
            .to_bytes();
        let signed = sample().sign("k1", &secret).unwrap();
        signed.verify_ed25519(&public).unwrap();
        assert_eq!(signed.trust, TrustLevel::IntegrityVerified);
    }

    #[test]
    fn digest_excludes_signature_value() {
        let secret = [7_u8; 32];
        let signed = sample().sign("k1", &secret).unwrap();
        let mut other = signed.clone();
        other.signature.value = "00".repeat(64);
        assert_eq!(signed.digest().unwrap(), other.digest().unwrap());
    }
}
