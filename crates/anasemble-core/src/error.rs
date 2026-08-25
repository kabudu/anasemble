//! Fail-closed core errors.

use thiserror::Error;

/// Encoding, bound, or identifier failure.
#[derive(Debug, Error)]
pub enum CoreError {
    /// JSON canonicalisation failed.
    #[error("canonical JSON encoding failed: {0}")]
    Json(#[from] serde_json::Error),
    /// Canonical CBOR could not be produced.
    #[error("{0}")]
    Cbor(&'static str),
    /// Identifier or text bound was violated.
    #[error(transparent)]
    Id(#[from] crate::ids::IdError),
    /// Digest hex was not 32 bytes.
    #[error("digest must be 32-byte lowercase hex")]
    InvalidDigest,
    /// Input exceeded a configured byte or depth ceiling.
    #[error("input exceeded a configured bound")]
    Bound,
    /// A named resource budget was exhausted.
    #[error("budget exhausted")]
    Budget,
}
