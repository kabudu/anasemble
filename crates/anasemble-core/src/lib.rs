//! Canonical encodings, content digests, and bounded identifiers.
//!
//! JSON encoding preserves existing Anasemble file compatibility. Canonical
//! CBOR is the signing and digest representation for generic envelopes.

#![forbid(unsafe_code)]

mod cbor;
mod error;
mod ids;
mod json;

pub use cbor::{MAX_CBOR_DEPTH, decode_to_json, encode_canonical_cbor};
pub use error::CoreError;
pub use ids::{BoundedText, IdError, MAX_BOUNDED_TEXT_BYTES, ObservationId, RunId, TenantId};
pub use json::{bytes_digest, digest_hex, encode_json};

use sha2::{Digest as _, Sha256};

/// 32-byte SHA-256 digest.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Digest([u8; 32]);

impl Digest {
    /// Hash already-canonical bytes.
    #[must_use]
    pub fn sha256(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut out = [0_u8; 32];
        out.copy_from_slice(&digest);
        Self(out)
    }

    /// Digest of canonical CBOR for `value`.
    pub fn of_canonical_cbor<T: serde::Serialize>(value: &T) -> Result<Self, CoreError> {
        Ok(Self::sha256(&encode_canonical_cbor(value)?))
    }

    /// Raw bytes.
    #[must_use]
    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    /// Lowercase hex.
    #[must_use]
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    /// Parse 64 lowercase hex characters.
    pub fn from_hex(value: &str) -> Result<Self, CoreError> {
        let bytes = hex::decode(value).map_err(|_| CoreError::InvalidDigest)?;
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| CoreError::InvalidDigest)?;
        Ok(Self(bytes))
    }
}

impl std::fmt::Display for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// Maximum accepted envelope or event framing size in bytes.
pub const MAX_FRAME_BYTES: usize = 1_048_576;

/// Maximum payload bytes after any bounded decompression.
pub const MAX_PAYLOAD_BYTES: usize = 524_288;
