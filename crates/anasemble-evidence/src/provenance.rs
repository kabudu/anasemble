//! Provenance graph nodes, edges, and revocation events.

use anasemble_core::{BoundedText, Digest, ObservationId};
use serde::{Deserialize, Serialize};

/// Node in the provenance DAG.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceNode {
    /// Content digest of the node payload.
    pub digest: Digest,
    /// Node kind, for example `envelope` or `certificate`.
    pub kind: BoundedText,
    /// Observation that produced the node, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation_id: Option<ObservationId>,
}

/// Directed provenance edge.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceEdge {
    /// Predecessor digest.
    pub from: Digest,
    /// Successor digest.
    pub to: Digest,
    /// Relationship, for example `derives` or `revokes`.
    pub relation: BoundedText,
}

/// Provenance event kinds. Revocation creates a new event; it does not erase history.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceEventKind {
    /// Evidence was accepted.
    Accepted,
    /// Evidence was rejected.
    Rejected,
    /// A signing or recovery key was revoked.
    KeyRevoked,
    /// A derived view must be rebuilt.
    Invalidated,
}

/// Append-only provenance event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceEvent {
    /// Event kind.
    pub kind: ProvenanceEventKind,
    /// When the event was recorded, RFC 3339 UTC.
    pub recorded_at: String,
    /// Subject digest.
    pub subject: Digest,
    /// Optional reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<BoundedText>,
}
