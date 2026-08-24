//! Ed25519 key status, revocation, and replay windows.

use std::collections::BTreeSet;

use anasemble_core::ObservationId;
use ed25519_dalek::SigningKey as Ed25519SigningKey;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::EvidenceError;
use crate::envelope::parse_time;

/// Signing secret. Zeroized on drop.
pub struct SigningKey {
    /// Key identifier.
    pub key_id: String,
    secret: [u8; 32],
}

impl SigningKey {
    /// Construct from raw bytes.
    #[must_use]
    pub fn from_bytes(key_id: impl Into<String>, secret: [u8; 32]) -> Self {
        Self {
            key_id: key_id.into(),
            secret,
        }
    }

    /// Corresponding public key bytes.
    #[must_use]
    pub fn public_bytes(&self) -> [u8; 32] {
        Ed25519SigningKey::from_bytes(&self.secret)
            .verifying_key()
            .to_bytes()
    }

    /// Borrow the secret for signing.
    #[must_use]
    pub fn secret_bytes(&self) -> &[u8; 32] {
        &self.secret
    }
}

impl Drop for SigningKey {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

/// Public key with validity and optional revocation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PublicKeyRecord {
    /// Key identifier.
    pub key_id: String,
    /// 32-byte public key, hex-encoded.
    pub public_key_hex: String,
    /// Inclusive validity start.
    pub not_before: String,
    /// Exclusive validity end.
    pub not_after: String,
    /// Revocation instant when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
}

/// Outcome of a key lookup at a verification time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyStatus {
    /// Key is valid at the verification time.
    Valid,
    /// Key is outside its validity interval.
    Expired,
    /// Key was revoked at or before verification time.
    Revoked,
    /// Key id is unknown.
    Unknown,
}

impl PublicKeyRecord {
    /// Status at `verification_time`.
    pub fn status_at(&self, verification_time: &str) -> Result<KeyStatus, EvidenceError> {
        let now = parse_time(verification_time)?;
        let not_before = parse_time(&self.not_before)?;
        let not_after = parse_time(&self.not_after)?;
        if let Some(revoked_at) = &self.revoked_at {
            let revoked = parse_time(revoked_at)?;
            if now.as_microsecond() >= revoked.as_microsecond() {
                return Ok(KeyStatus::Revoked);
            }
        }
        if now.as_microsecond() < not_before.as_microsecond()
            || now.as_microsecond() >= not_after.as_microsecond()
        {
            return Ok(KeyStatus::Expired);
        }
        Ok(KeyStatus::Valid)
    }

    /// Decode the public key.
    pub fn public_bytes(&self) -> Result<[u8; 32], EvidenceError> {
        hex::decode(&self.public_key_hex)
            .map_err(|_| EvidenceError::Crypto("public key is not hex"))?
            .try_into()
            .map_err(|_| EvidenceError::Crypto("public key is not 32 bytes"))
    }
}

/// Replay window keyed by observation id.
#[derive(Clone, Debug, Default)]
pub struct ReplayWindow {
    seen: BTreeSet<ObservationId>,
    max_entries: usize,
}

impl ReplayWindow {
    /// Construct a bounded window.
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            seen: BTreeSet::new(),
            max_entries,
        }
    }

    /// Accept an observation id once. Duplicates are replays.
    pub fn accept(&mut self, observation_id: ObservationId) -> Result<(), EvidenceError> {
        if !self.seen.insert(observation_id) {
            return Err(EvidenceError::Rejected("replayed observation"));
        }
        if self.seen.len() > self.max_entries {
            return Err(EvidenceError::Rejected("replay window exhausted"));
        }
        Ok(())
    }

    /// Number of remembered observations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// Whether the window is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}
