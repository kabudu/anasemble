//! Generic envelope and event protocol. Existing fragment files stay on JSON HMAC/Ed25519.

use anasemble::fragments::{Envelope, FragmentKind, sign};
use anasemble::model::FragmentContent;
use anasemble::{
    Digest, EventLog, EvidenceEnvelopeV1, EvidenceKind, ObservationId, PublicKeyRecord,
    ReconstructionEventKind, ReconstructionEventV1, ReplayWindow, RunId, SignatureBlock,
    SigningKey, TenantId, TrustLevel, canonical, encode_canonical_cbor, validate_envelope,
};
use anasemble_core::BoundedText;
use anasemble_evidence::{
    CollectionContext, RedactablePayload, SourceIdentity, SubjectRef, ValidInterval,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct CompatibilitySample {
    b: u8,
    a: u8,
}

#[test]
fn historical_json_canonical_digest_is_unchanged() {
    let encoded = canonical::encode(&CompatibilitySample { b: 2, a: 1 }).unwrap();
    assert_eq!(encoded, br#"{"a":1,"b":2}"#);
    assert_eq!(
        canonical::digest(&serde_json::json!({})).unwrap(),
        "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
    );
}

#[test]
fn existing_fragment_envelope_still_signs() {
    let envelope = Envelope {
        kind: FragmentKind::Contract,
        component: "demo".into(),
        interface_version: "v1".into(),
        issuer: "issuer".into(),
        failure_domain: "domain".into(),
        issued_at: "2026-01-01T00:00:00+00:00".into(),
        sequence: 1,
        content_digest: String::new(),
        dependencies: Vec::new(),
        content: FragmentContent::Transition {
            state: "A".into(),
            input: "go".into(),
            next_state: "B".into(),
            output: "ok".into(),
        },
        signature: String::new(),
    };
    let signed = sign(envelope, &[7_u8; 32]).unwrap();
    assert!(!signed.signature.is_empty());
    assert!(!signed.content_digest.is_empty());
}

fn sample_envelope(tenant: TenantId, signing: &SigningKey) -> EvidenceEnvelopeV1 {
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

#[test]
fn generic_envelope_golden_cbor_empty_map() {
    assert_eq!(
        encode_canonical_cbor(&serde_json::json!({})).unwrap(),
        vec![0xa0]
    );
}

#[test]
fn generic_envelope_admits_and_replays() {
    let tenant = TenantId::generate();
    let signing = SigningKey::from_bytes("k1", [11_u8; 32]);
    let envelope = sample_envelope(tenant, &signing);
    let json = serde_json::to_vec(&envelope).unwrap();
    let mut keys = BTreeMap::new();
    keys.insert(
        "k1".into(),
        PublicKeyRecord {
            key_id: "k1".into(),
            public_key_hex: hex::encode(signing.public_bytes()),
            not_before: "2026-01-01T00:00:00Z".into(),
            not_after: "2027-01-01T00:00:00Z".into(),
            revoked_at: None,
        },
    );
    let mut replay = ReplayWindow::new(8);
    validate_envelope(&json, tenant, &keys, &mut replay, "2026-08-24T22:00:02Z").unwrap();
    assert!(validate_envelope(&json, tenant, &keys, &mut replay, "2026-08-24T22:00:02Z").is_err());
}

#[test]
fn reconstruction_events_are_gap_closed() {
    let signing = SigningKey::from_bytes("k1", [5_u8; 32]);
    let public = signing.public_bytes();
    let run = RunId::generate();
    let mut log = EventLog::new();
    let first = ReconstructionEventV1 {
        schema_version: 1,
        run_id: run,
        sequence: 1,
        predecessor: None,
        timestamp: "2026-08-24T22:00:00Z".into(),
        actor: BoundedText::new("operator").unwrap(),
        kind: ReconstructionEventKind::ReconstructionStarted,
        payload_digest: Digest::sha256(b"start"),
        signature: SignatureBlock {
            algorithm: BoundedText::new("ed25519").unwrap(),
            key_id: BoundedText::new("k1").unwrap(),
            value: String::new(),
        },
    };
    log.append(first, &signing, &public).unwrap();
    assert_eq!(log.head(), Some((run, 1)));
}
