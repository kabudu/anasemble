//! Append-only reconstruction and certification events.
//!
//! Consumers resume by `(run_id, sequence)` and refuse gaps. This protocol is
//! generic; Anasemble's existing CLI receipts remain unchanged.

#![forbid(unsafe_code)]

use anasemble_core::{BoundedText, Digest, RunId, encode_canonical_cbor};
use anasemble_evidence::{SignatureBlock, SigningKey};
use ed25519_dalek::{Signature, Signer, SigningKey as Ed25519SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Event schema version.
pub const SCHEMA_VERSION: u16 = 1;

/// Fail-closed event log error.
#[derive(Debug, Error)]
pub enum EventError {
    /// Core encoding failure.
    #[error(transparent)]
    Core(#[from] anasemble_core::CoreError),
    #[error(transparent)]
    Id(#[from] anasemble_core::IdError),
    /// Evidence signing types failed.
    #[error(transparent)]
    Evidence(#[from] anasemble_evidence::EvidenceError),
    /// Sequence or predecessor did not match the log.
    #[error("{0}")]
    Gap(&'static str),
    /// Signature verification failed.
    #[error("{0}")]
    Crypto(&'static str),
}

/// Reconstruction and certification lifecycle events.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconstructionEventKind {
    /// A reconstruction run started.
    ReconstructionStarted,
    /// Evidence was accepted.
    EvidenceAccepted,
    /// Evidence was rejected.
    EvidenceRejected,
    /// Canonical bytes were produced.
    Canonicalised,
    /// A candidate was produced.
    CandidateProduced,
    /// A verification claim was evaluated.
    VerificationClaimEvaluated,
    /// A certificate was issued.
    Certified,
    /// Certification was refused.
    Refused,
    /// A state restore was planned.
    StateRestorePlanned,
    /// A state restore was applied.
    StateRestoreApplied,
    /// A state restore was rolled back.
    StateRestoreRolledBack,
    /// Activation was staged.
    ActivationStaged,
    /// Activation was committed.
    ActivationCommitted,
    /// Activation was rolled back.
    ActivationRolledBack,
    /// The run completed.
    RunCompleted,
}

/// Signed reconstruction event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReconstructionEventV1 {
    /// Schema version.
    pub schema_version: u16,
    /// Run identity.
    pub run_id: RunId,
    /// Monotonic sequence starting at 1.
    pub sequence: u64,
    /// Digest of the previous event, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predecessor: Option<Digest>,
    /// Event time, RFC 3339 UTC.
    pub timestamp: String,
    /// Actor identity.
    pub actor: BoundedText,
    /// Event kind.
    pub kind: ReconstructionEventKind,
    /// Digest of the event-specific payload bytes.
    pub payload_digest: Digest,
    /// Ed25519 signature over canonical CBOR of the unsigned event.
    pub signature: SignatureBlock,
}

#[derive(Serialize)]
struct UnsignedEvent<'a> {
    schema_version: u16,
    run_id: RunId,
    sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    predecessor: Option<Digest>,
    timestamp: &'a str,
    actor: &'a BoundedText,
    kind: ReconstructionEventKind,
    payload_digest: Digest,
}

impl ReconstructionEventV1 {
    fn unsigned(&self) -> UnsignedEvent<'_> {
        UnsignedEvent {
            schema_version: self.schema_version,
            run_id: self.run_id,
            sequence: self.sequence,
            predecessor: self.predecessor,
            timestamp: &self.timestamp,
            actor: &self.actor,
            kind: self.kind,
            payload_digest: self.payload_digest,
        }
    }

    /// Canonical CBOR preimage.
    pub fn canonical_cbor(&self) -> Result<Vec<u8>, EventError> {
        Ok(encode_canonical_cbor(&self.unsigned())?)
    }

    /// Digest of the unsigned event.
    pub fn digest(&self) -> Result<Digest, EventError> {
        Ok(Digest::sha256(&self.canonical_cbor()?))
    }

    /// Sign the event.
    pub fn sign(mut self, signing: &SigningKey) -> Result<Self, EventError> {
        self.signature = SignatureBlock {
            algorithm: BoundedText::new("ed25519")?,
            key_id: BoundedText::new(signing.key_id.as_str())?,
            value: String::new(),
        };
        let signature =
            Ed25519SigningKey::from_bytes(signing.secret_bytes()).sign(&self.canonical_cbor()?);
        self.signature.value = hex::encode(signature.to_bytes());
        Ok(self)
    }

    /// Verify the event signature.
    pub fn verify(&self, public: &[u8; 32]) -> Result<(), EventError> {
        if self.signature.algorithm.as_str() != "ed25519" {
            return Err(EventError::Crypto("unsupported signature algorithm"));
        }
        let bytes = hex::decode(&self.signature.value)
            .map_err(|_| EventError::Crypto("signature is not hex"))?;
        let bytes: [u8; 64] = bytes
            .try_into()
            .map_err(|_| EventError::Crypto("signature is not 64 bytes"))?;
        VerifyingKey::from_bytes(public)
            .map_err(|_| EventError::Crypto("public key is invalid"))?
            .verify(&self.canonical_cbor()?, &Signature::from_bytes(&bytes))
            .map_err(|_| EventError::Crypto("signature verification failed"))
    }
}

/// In-memory gap-detecting log. Durable persistence is a consumer concern.
#[derive(Clone, Debug, Default)]
pub struct EventLog {
    events: Vec<ReconstructionEventV1>,
}

impl EventLog {
    /// Empty log.
    #[must_use]
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Append a signed event. Sequence must be last+1 and predecessor must match.
    pub fn append(
        &mut self,
        mut event: ReconstructionEventV1,
        signing: &SigningKey,
        public: &[u8; 32],
    ) -> Result<ReconstructionEventV1, EventError> {
        if event.schema_version != SCHEMA_VERSION {
            return Err(EventError::Gap("unsupported event schema"));
        }
        let expected_seq = u64::try_from(self.events.len()).expect("event log length fits u64") + 1;
        if event.sequence != expected_seq {
            return Err(EventError::Gap("event sequence gap"));
        }
        let expected_pred = self
            .events
            .last()
            .map(|previous| previous.digest())
            .transpose()?;
        if event.predecessor != expected_pred {
            return Err(EventError::Gap("event predecessor mismatch"));
        }
        event = event.sign(signing)?;
        event.verify(public)?;
        self.events.push(event.clone());
        Ok(event)
    }

    /// Resume cursor.
    #[must_use]
    pub fn head(&self) -> Option<(RunId, u64)> {
        self.events
            .last()
            .map(|event| (event.run_id, event.sequence))
    }

    /// Borrow events in order.
    #[must_use]
    pub fn events(&self) -> &[ReconstructionEventV1] {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anasemble_core::BoundedText;
    use anasemble_evidence::{SignatureBlock, SigningKey};

    fn unsigned(run: RunId, sequence: u64, predecessor: Option<Digest>) -> ReconstructionEventV1 {
        ReconstructionEventV1 {
            schema_version: 1,
            run_id: run,
            sequence,
            predecessor,
            timestamp: "2026-08-24T22:00:00Z".into(),
            actor: BoundedText::new("operator").unwrap(),
            kind: ReconstructionEventKind::ReconstructionStarted,
            payload_digest: Digest::sha256(b"start"),
            signature: SignatureBlock {
                algorithm: BoundedText::new("ed25519").unwrap(),
                key_id: BoundedText::new("k1").unwrap(),
                value: String::new(),
            },
        }
    }

    #[test]
    fn append_detects_gaps() {
        let signing = SigningKey::from_bytes("k1", [5_u8; 32]);
        let public = signing.public_bytes();
        let run = RunId::generate();
        let mut log = EventLog::new();
        log.append(unsigned(run, 1, None), &signing, &public)
            .unwrap();
        assert!(
            log.append(unsigned(run, 3, None), &signing, &public)
                .is_err()
        );
        let pred = log.events()[0].digest().unwrap();
        log.append(unsigned(run, 2, Some(pred)), &signing, &public)
            .unwrap();
        assert_eq!(log.head(), Some((run, 2)));
    }

    #[test]
    fn refuses_wrong_predecessor() {
        let signing = SigningKey::from_bytes("k1", [5_u8; 32]);
        let public = signing.public_bytes();
        let run = RunId::generate();
        let mut log = EventLog::new();
        log.append(unsigned(run, 1, None), &signing, &public)
            .unwrap();
        let bogus = Digest::sha256(b"nope");
        assert!(
            log.append(unsigned(run, 2, Some(bogus)), &signing, &public)
                .is_err()
        );
    }
}
