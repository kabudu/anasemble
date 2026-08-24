//! Streaming admission in the documented validation order.

use std::collections::BTreeMap;
use std::io::Read;

use anasemble_core::{MAX_FRAME_BYTES, decode_to_json};
use serde::{Deserialize, Serialize};

use crate::EvidenceError;
use crate::envelope::{EvidenceEnvelopeV1, EvidenceKind, PayloadClass, TrustLevel};
use crate::keys::{KeyStatus, PublicKeyRecord, ReplayWindow};

/// Why an envelope was refused. Unsafe payload bytes are not retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    /// Framing exceeded the byte ceiling.
    SizeLimit,
    /// Schema version is unsupported.
    SchemaVersion,
    /// Tenant or source binding failed.
    TenantBinding,
    /// Signature, key status, or algorithm failed.
    Signature,
    /// Observation was replayed.
    Replay,
    /// Timestamp was invalid.
    Timestamp,
    /// Media type or digest failed.
    Payload,
    /// Label or field limits failed.
    Labels,
    /// Payload class is forbidden.
    Redaction,
    /// Kind/trust combination is inadmissible.
    Taxonomy,
}

/// Admission outcome.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionDecision {
    /// Envelope may proceed.
    Accept,
    /// Envelope is refused.
    Reject {
        /// Machine-readable reason.
        reason: RejectionReason,
    },
}

/// Validate CBOR or JSON framing then apply admission policy.
pub fn validate_envelope(
    frame: &[u8],
    authenticated_tenant: anasemble_core::TenantId,
    keys: &BTreeMap<String, PublicKeyRecord>,
    replay: &mut ReplayWindow,
    verification_time: &str,
) -> Result<EvidenceEnvelopeV1, EvidenceError> {
    if frame.len() > MAX_FRAME_BYTES || frame.is_empty() {
        return Err(EvidenceError::Rejected("envelope exceeds size limit"));
    }
    let envelope = decode_envelope(frame)?;
    match admit(
        &envelope,
        authenticated_tenant,
        keys,
        replay,
        verification_time,
    ) {
        AdmissionDecision::Accept => Ok(envelope),
        AdmissionDecision::Reject { reason } => Err(map_reason(reason)),
    }
}

/// Read a bounded frame from `reader`.
pub fn read_bounded_frame(reader: &mut impl Read, limit: usize) -> Result<Vec<u8>, EvidenceError> {
    let mut buf = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|_| EvidenceError::Rejected("envelope read failed"))?;
        if n == 0 {
            break;
        }
        if buf.len().saturating_add(n) > limit {
            return Err(EvidenceError::Rejected("envelope exceeds size limit"));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    if buf.is_empty() {
        return Err(EvidenceError::Rejected("envelope exceeds size limit"));
    }
    Ok(buf)
}

fn decode_envelope(frame: &[u8]) -> Result<EvidenceEnvelopeV1, EvidenceError> {
    if frame.first() == Some(&b'{') {
        return serde_json::from_slice(frame)
            .map_err(|_| EvidenceError::Rejected("envelope JSON is invalid"));
    }
    let value =
        decode_to_json(frame).map_err(|_| EvidenceError::Rejected("envelope CBOR is invalid"))?;
    serde_json::from_value(value).map_err(|_| EvidenceError::Rejected("envelope CBOR is invalid"))
}

fn admit(
    envelope: &EvidenceEnvelopeV1,
    authenticated_tenant: anasemble_core::TenantId,
    keys: &BTreeMap<String, PublicKeyRecord>,
    replay: &mut ReplayWindow,
    verification_time: &str,
) -> AdmissionDecision {
    if envelope.schema_version != 1 {
        return reject(RejectionReason::SchemaVersion);
    }
    if envelope.tenant_id != authenticated_tenant {
        return reject(RejectionReason::TenantBinding);
    }
    let key = match keys.get(envelope.signature.key_id.as_str()) {
        Some(key) => key,
        None => return reject(RejectionReason::Signature),
    };
    match key.status_at(verification_time) {
        Ok(KeyStatus::Valid) => {}
        Ok(KeyStatus::Revoked | KeyStatus::Expired | KeyStatus::Unknown) => {
            return reject(RejectionReason::Signature);
        }
        Err(_) => return reject(RejectionReason::Timestamp),
    }
    let public = match key.public_bytes() {
        Ok(public) => public,
        Err(_) => return reject(RejectionReason::Signature),
    };
    if envelope.verify_ed25519(&public).is_err() {
        return reject(RejectionReason::Signature);
    }
    if replay.accept(envelope.observation_id).is_err() {
        return reject(RejectionReason::Replay);
    }
    if envelope.check_bounds().is_err() {
        if envelope.valid.validate().is_err()
            || crate::envelope::parse_time(&envelope.observed_at).is_err()
        {
            return reject(RejectionReason::Timestamp);
        }
        if matches!(envelope.payload.class(), PayloadClass::Forbidden) {
            return reject(RejectionReason::Redaction);
        }
        return reject(RejectionReason::Payload);
    }
    if envelope.labels.len() > crate::MAX_LABELS {
        return reject(RejectionReason::Labels);
    }
    if matches!(envelope.kind, EvidenceKind::Verified)
        && !matches!(
            envelope.trust,
            TrustLevel::Certified
                | TrustLevel::IndependentlyCorroborated
                | TrustLevel::IntegrityVerified
        )
    {
        return reject(RejectionReason::Taxonomy);
    }
    AdmissionDecision::Accept
}

fn reject(reason: RejectionReason) -> AdmissionDecision {
    AdmissionDecision::Reject { reason }
}

fn map_reason(reason: RejectionReason) -> EvidenceError {
    match reason {
        RejectionReason::SizeLimit => EvidenceError::Rejected("envelope exceeds size limit"),
        RejectionReason::SchemaVersion => EvidenceError::Rejected("unsupported schema version"),
        RejectionReason::TenantBinding => EvidenceError::Rejected("tenant binding mismatch"),
        RejectionReason::Signature => EvidenceError::Rejected("signature or key is invalid"),
        RejectionReason::Replay => EvidenceError::Rejected("replayed observation"),
        RejectionReason::Timestamp => EvidenceError::Rejected("timestamp is invalid"),
        RejectionReason::Payload => EvidenceError::Rejected("payload digest or media type failed"),
        RejectionReason::Labels => EvidenceError::Rejected("labels exceed policy"),
        RejectionReason::Redaction => EvidenceError::Rejected("payload class is forbidden"),
        RejectionReason::Taxonomy => EvidenceError::Rejected("kind and trust are inadmissible"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{
        CollectionContext, RedactablePayload, SignatureBlock, SourceIdentity, SubjectRef,
        ValidInterval,
    };
    use crate::keys::SigningKey;
    use anasemble_core::{BoundedText, Digest, ObservationId, TenantId};
    use std::io::Cursor;

    fn signed_envelope(tenant: TenantId, signing: &SigningKey) -> EvidenceEnvelopeV1 {
        let payload = b"{\"ok\":true}";
        EvidenceEnvelopeV1 {
            schema_version: 1,
            observation_id: ObservationId::generate(),
            tenant_id: tenant,
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
                key_id: BoundedText::new(&signing.key_id).unwrap(),
                value: String::new(),
            },
        }
        .sign(&signing.key_id, signing.secret_bytes())
        .unwrap()
    }

    fn key_record(signing: &SigningKey) -> PublicKeyRecord {
        PublicKeyRecord {
            key_id: signing.key_id.clone(),
            public_key_hex: hex::encode(signing.public_bytes()),
            not_before: "2026-01-01T00:00:00Z".into(),
            not_after: "2027-01-01T00:00:00Z".into(),
            revoked_at: None,
        }
    }

    #[test]
    fn accepts_signed_json() {
        let tenant = TenantId::generate();
        let signing = SigningKey::from_bytes("k1", [11_u8; 32]);
        let envelope = signed_envelope(tenant, &signing);
        let json = serde_json::to_vec(&envelope).unwrap();
        let mut keys = BTreeMap::new();
        keys.insert("k1".into(), key_record(&signing));
        let mut replay = ReplayWindow::new(16);
        validate_envelope(&json, tenant, &keys, &mut replay, "2026-08-24T22:00:02Z").unwrap();
    }

    #[test]
    fn rejects_replay() {
        let tenant = TenantId::generate();
        let signing = SigningKey::from_bytes("k1", [11_u8; 32]);
        let envelope = signed_envelope(tenant, &signing);
        let json = serde_json::to_vec(&envelope).unwrap();
        let mut keys = BTreeMap::new();
        keys.insert("k1".into(), key_record(&signing));
        let mut replay = ReplayWindow::new(16);
        validate_envelope(&json, tenant, &keys, &mut replay, "2026-08-24T22:00:02Z").unwrap();
        assert!(
            validate_envelope(&json, tenant, &keys, &mut replay, "2026-08-24T22:00:02Z").is_err()
        );
    }

    #[test]
    fn rejects_revoked_key() {
        let tenant = TenantId::generate();
        let signing = SigningKey::from_bytes("k1", [11_u8; 32]);
        let envelope = signed_envelope(tenant, &signing);
        let json = serde_json::to_vec(&envelope).unwrap();
        let mut record = key_record(&signing);
        record.revoked_at = Some("2026-08-01T00:00:00Z".into());
        let mut keys = BTreeMap::new();
        keys.insert("k1".into(), record);
        let mut replay = ReplayWindow::new(16);
        assert!(
            validate_envelope(&json, tenant, &keys, &mut replay, "2026-08-24T22:00:02Z").is_err()
        );
    }

    #[test]
    fn rejects_cross_tenant() {
        let signing = SigningKey::from_bytes("k1", [11_u8; 32]);
        let envelope = signed_envelope(TenantId::generate(), &signing);
        let json = serde_json::to_vec(&envelope).unwrap();
        let mut keys = BTreeMap::new();
        keys.insert("k1".into(), key_record(&signing));
        let mut replay = ReplayWindow::new(16);
        assert!(
            validate_envelope(
                &json,
                TenantId::generate(),
                &keys,
                &mut replay,
                "2026-08-24T22:00:02Z"
            )
            .is_err()
        );
    }

    #[test]
    fn mutation_of_signed_bytes_is_rejected() {
        let tenant = TenantId::generate();
        let signing = SigningKey::from_bytes("k1", [11_u8; 32]);
        let envelope = signed_envelope(tenant, &signing);
        let mut json = serde_json::to_vec(&envelope).unwrap();
        let mut keys = BTreeMap::new();
        keys.insert("k1".into(), key_record(&signing));
        let mut accepted = 0_u32;
        for i in 0..json.len() {
            let original = json[i];
            json[i] ^= 0xff;
            let mut replay = ReplayWindow::new(16);
            if validate_envelope(&json, tenant, &keys, &mut replay, "2026-08-24T22:00:02Z").is_ok()
            {
                accepted += 1;
            }
            json[i] = original;
        }
        assert_eq!(accepted, 0);
    }

    #[test]
    fn bounded_reader_refuses_oversize() {
        let mut cursor = Cursor::new(vec![0_u8; 32]);
        assert!(read_bounded_frame(&mut cursor, 8).is_err());
    }
}
