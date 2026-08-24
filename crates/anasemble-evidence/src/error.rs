//! Evidence crate errors.

use thiserror::Error;

/// Fail-closed evidence protocol error.
#[derive(Debug, Error)]
pub enum EvidenceError {
    /// Core encoding or identifier failure.
    #[error(transparent)]
    Core(#[from] anasemble_core::CoreError),
    /// Identifier or text bound failed.
    #[error(transparent)]
    Id(#[from] anasemble_core::IdError),
    /// Envelope was refused by admission policy.
    #[error("{0}")]
    Rejected(&'static str),
    /// Cryptographic operation failed.
    #[error("{0}")]
    Crypto(&'static str),
    /// Timestamp was not a UTC instant.
    #[error("timestamp is not a valid UTC instant")]
    Timestamp,
}
