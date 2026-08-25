use serde::Serialize;

use crate::Error;

/// Canonical JSON encoding. Unchanged from the pre-extraction path so existing
/// files and certificates keep their digests.
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    anasemble_core::encode_json(value).map_err(map_core)
}

/// SHA-256 hex digest of canonical JSON.
pub fn digest<T: Serialize>(value: &T) -> Result<String, Error> {
    anasemble_core::digest_hex(value).map_err(map_core)
}

/// SHA-256 hex digest of raw bytes.
#[must_use]
pub fn bytes_digest(value: &[u8]) -> String {
    anasemble_core::bytes_digest(value)
}

fn map_core(error: anasemble_core::CoreError) -> Error {
    match error {
        anasemble_core::CoreError::Json(error) => Error::Json(error),
        other => Error::InvalidEvidence(other.to_string()),
    }
}
