//! Canonical JSON encoding used by existing Anasemble files.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::CoreError;

/// Encode `value` as canonical JSON bytes.
///
/// Compatibility: this is the historical Anasemble digest input. Do not change
/// it without a dual-read migration.
pub fn encode_json<T: Serialize>(value: &T) -> Result<Vec<u8>, CoreError> {
    let canonical_tree = serde_json::to_value(value)?;
    Ok(serde_json::to_vec(&canonical_tree)?)
}

/// SHA-256 hex digest of canonical JSON.
pub fn digest_hex<T: Serialize>(value: &T) -> Result<String, CoreError> {
    Ok(hex::encode(Sha256::digest(encode_json(value)?)))
}

/// SHA-256 hex digest of raw bytes.
#[must_use]
pub fn bytes_digest(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Sample {
        b: u8,
        a: u8,
    }

    #[test]
    fn json_object_keys_are_sorted() {
        let encoded = encode_json(&Sample { b: 2, a: 1 }).unwrap();
        assert_eq!(encoded, br#"{"a":1,"b":2}"#);
    }

    #[test]
    fn empty_object_digest_is_stable() {
        let digest = digest_hex(&serde_json::json!({})).unwrap();
        assert_eq!(
            digest,
            "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        );
    }
}
