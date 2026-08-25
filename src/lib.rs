//! Bounded service-reconstruction research harness.

pub mod activation;
pub mod brand;
pub mod campaign;
pub mod canonical;
pub mod checker;
pub mod checker_wire;
pub mod corpus;
pub mod deployment;
pub mod evidence_plane;
pub mod fragments;
pub mod ledger;
pub mod lifecycle;
pub mod model;
pub mod operations;
pub mod oracle;
pub mod protocol;
pub mod reference;
pub mod sandbox;
pub mod service;
pub mod state_store;
pub mod stateful;
pub mod synthesizer;

pub use model::{Error, RefusalCode};

pub use anasemble_contract::ContractV1;
pub use anasemble_core::{
    BoundedText, Budget, CompatibilityClass, Digest, ObservationId, RunId, TemporalCut, TenantId,
    TraceId, bytes_digest, digest_hex, encode_canonical_cbor, encode_json,
};
pub use anasemble_events::{EventLog, ReconstructionEventKind, ReconstructionEventV1};
pub use anasemble_evidence::{
    EvidenceEnvelopeV1, EvidenceKind, ProvenanceEvent, ProvenanceEventKind, PublicKeyRecord,
    ReplayWindow, SignatureBlock, SigningKey, TrustLevel, validate_envelope,
};
pub use anasemble_semantic::{
    CanonicalDiffer, DiffPolicy, GraphSnapshotter, SemanticDiff, SemanticDiffer, SemanticGraph,
    SemanticSnapshotV1, SemanticSnapshotter,
};
pub use anasemble_trace::{Recorder, Replayer, TraceV1};
pub use anasemble_verification::{
    CertificateV1, DigestCandidate, DigestEqualityVerifier, VerificationReport, Verifier,
};
