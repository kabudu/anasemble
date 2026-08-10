use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::Error;

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    let canonical_tree = serde_json::to_value(value)?;
    Ok(serde_json::to_vec(&canonical_tree)?)
}

pub fn digest<T: Serialize>(value: &T) -> Result<String, Error> {
    Ok(hex::encode(Sha256::digest(encode(value)?)))
}

#[must_use]
pub fn bytes_digest(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}
