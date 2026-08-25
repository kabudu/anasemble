//! Contract identity, semantic version, predecessor, and migration obligations.

#![forbid(unsafe_code)]

use anasemble_core::{BoundedText, CompatibilityClass, ContractId, Digest, encode_canonical_cbor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Contract schema version.
pub const SCHEMA_VERSION: u16 = 1;

/// Fail-closed contract error.
#[derive(Debug, Error)]
pub enum ContractError {
    #[error(transparent)]
    Core(#[from] anasemble_core::CoreError),
    #[error(transparent)]
    Id(#[from] anasemble_core::IdError),
    #[error("{0}")]
    Protocol(&'static str),
}

/// Versioned contract. Predecessor is required after the first version.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContractV1 {
    /// Schema version.
    pub schema_version: u16,
    /// Stable identity across versions.
    pub contract_id: ContractId,
    /// Semantic version text, for example `1.2.0`.
    pub version: BoundedText,
    /// Previous contract identity when this is not the genesis version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predecessor: Option<ContractId>,
    /// Declared compatibility with the predecessor.
    pub compatibility: CompatibilityClass,
    /// Required migrations before the new version may replace the old.
    pub migration_obligations: Vec<BoundedText>,
    /// Canonical digest of the unsigned contract.
    pub digest: Digest,
}

#[derive(Serialize)]
struct UnsignedContract<'a> {
    schema_version: u16,
    contract_id: ContractId,
    version: &'a BoundedText,
    #[serde(skip_serializing_if = "Option::is_none")]
    predecessor: Option<ContractId>,
    compatibility: CompatibilityClass,
    migration_obligations: &'a [BoundedText],
}

impl ContractV1 {
    /// Construct and bind the digest. Breaking changes require a predecessor.
    pub fn new(
        version: &str,
        predecessor: Option<ContractId>,
        compatibility: CompatibilityClass,
        migration_obligations: Vec<BoundedText>,
    ) -> Result<Self, ContractError> {
        if predecessor.is_none() && compatibility != CompatibilityClass::Backward {
            return Err(ContractError::Protocol(
                "genesis contract must be backward compatible with itself",
            ));
        }
        if matches!(compatibility, CompatibilityClass::Breaking) && migration_obligations.is_empty()
        {
            return Err(ContractError::Protocol(
                "breaking change requires a migration obligation",
            ));
        }
        let mut contract = Self {
            schema_version: SCHEMA_VERSION,
            contract_id: ContractId::generate(),
            version: BoundedText::new(version)?,
            predecessor,
            compatibility,
            migration_obligations,
            digest: Digest::sha256(b""),
        };
        contract.digest = contract.compute_digest()?;
        Ok(contract)
    }

    /// Canonical digest.
    pub fn compute_digest(&self) -> Result<Digest, ContractError> {
        Ok(Digest::sha256(&encode_canonical_cbor(&UnsignedContract {
            schema_version: self.schema_version,
            contract_id: self.contract_id,
            version: &self.version,
            predecessor: self.predecessor,
            compatibility: self.compatibility,
            migration_obligations: &self.migration_obligations,
        })?))
    }
}

/// Independent digest checker.
pub fn independent_contract_digest(contract: &ContractV1) -> Result<Digest, ContractError> {
    contract.compute_digest()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_and_breaking_rules() {
        let genesis =
            ContractV1::new("1.0.0", None, CompatibilityClass::Backward, Vec::new()).unwrap();
        assert_eq!(
            independent_contract_digest(&genesis).unwrap(),
            genesis.digest
        );
        assert!(
            ContractV1::new(
                "1.1.0",
                Some(genesis.contract_id),
                CompatibilityClass::Breaking,
                Vec::new()
            )
            .is_err()
        );
        let breaking = ContractV1::new(
            "2.0.0",
            Some(genesis.contract_id),
            CompatibilityClass::Breaking,
            vec![BoundedText::new("migrate-rows").unwrap()],
        )
        .unwrap();
        assert_eq!(breaking.predecessor, Some(genesis.contract_id));
    }
}
