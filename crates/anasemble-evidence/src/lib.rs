//! Generic evidence envelopes, provenance, Ed25519 signatures, and XChaCha20-Poly1305 seals.
//!
//! This crate is the Anasemble-owned wire protocol Threniq consumes. Existing
//! Anasemble fragment `Envelope` files remain supported by the root crate.

#![forbid(unsafe_code)]

mod envelope;
mod error;
mod keys;
mod provenance;
mod seal;
mod validate;

pub use envelope::{
    CollectionContext, EvidenceEnvelopeV1, EvidenceKind, PayloadClass, RedactablePayload,
    SCHEMA_VERSION, SignatureBlock, SourceIdentity, SubjectRef, TrustLevel, ValidInterval,
};
pub use error::EvidenceError;
pub use keys::{KeyStatus, PublicKeyRecord, ReplayWindow, SigningKey};
pub use provenance::{ProvenanceEdge, ProvenanceEvent, ProvenanceEventKind, ProvenanceNode};
pub use seal::{SealedPayload, seal_payload, unseal_payload};
pub use validate::{AdmissionDecision, RejectionReason, read_bounded_frame, validate_envelope};

/// Maximum labels on a generic envelope.
pub const MAX_LABELS: usize = 32;
/// Maximum media type bytes.
pub const MAX_MEDIA_TYPE_BYTES: usize = 256;
