//! Protocol temporal cut. Both axes are required.

use serde::{Deserialize, Serialize};

/// Query cut naming valid time and knowledge time as RFC 3339 UTC.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TemporalCut {
    /// When the fact is asserted to hold.
    pub valid_at: String,
    /// When the producer had accepted the records used to answer.
    pub system_as_of: String,
}

impl TemporalCut {
    /// Bind both axes. Empty axis names are refused later by consumers.
    #[must_use]
    pub fn new(valid_at: impl Into<String>, system_as_of: impl Into<String>) -> Self {
        Self {
            valid_at: valid_at.into(),
            system_as_of: system_as_of.into(),
        }
    }
}

/// Compatibility class for contract and diff operations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityClass {
    /// New version can replace the old.
    Backward,
    /// Old version can still consume the new.
    Forward,
    /// Explicit incompatibility.
    Breaking,
    /// Classification was not possible under the budget.
    Unknown,
}
