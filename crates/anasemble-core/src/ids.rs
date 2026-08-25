//! UUIDv7 identifiers and bounded UTF-8 text.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::{Uuid, Version};

/// Identifier construction error.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum IdError {
    /// The UUID is not version 7.
    #[error("identifier must be UUIDv7")]
    NotUuidV7,
    /// Text was empty or exceeded the byte ceiling.
    #[error("bounded text must be non-empty and at most {0} bytes")]
    TextBound(usize),
}

/// Maximum UTF-8 bytes for a bounded text field.
pub const MAX_BOUNDED_TEXT_BYTES: usize = 256;

/// Non-empty UTF-8 text with a hard byte ceiling.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BoundedText(String);

impl BoundedText {
    /// Construct text that fits [`MAX_BOUNDED_TEXT_BYTES`].
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_BOUNDED_TEXT_BYTES {
            return Err(IdError::TextBound(MAX_BOUNDED_TEXT_BYTES));
        }
        Ok(Self(value))
    }

    /// Borrow the text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for BoundedText {
    type Error = IdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<BoundedText> for String {
    fn from(value: BoundedText) -> Self {
        value.0
    }
}

impl fmt::Display for BoundedText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

macro_rules! uuid_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(
            Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Allocate a new UUIDv7 identifier.
            #[must_use]
            pub fn generate() -> Self {
                Self(Uuid::now_v7())
            }

            /// Wrap an existing UUIDv7.
            pub fn from_uuid(uuid: Uuid) -> Result<Self, IdError> {
                if uuid.get_version() != Some(Version::SortRand) {
                    return Err(IdError::NotUuidV7);
                }
                Ok(Self(uuid))
            }

            /// Return the inner UUID.
            #[must_use]
            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let uuid = Uuid::parse_str(s).map_err(|_| IdError::NotUuidV7)?;
                Self::from_uuid(uuid)
            }
        }
    };
}

uuid_id!(TenantId, "Tenant bound into every generic envelope.");
uuid_id!(
    ObservationId,
    "Accepted or rejected observation identifier."
);
uuid_id!(RunId, "Reconstruction or certification run identifier.");
uuid_id!(TraceId, "Behavioural trace identifier.");
uuid_id!(SnapshotId, "Canonical semantic snapshot identifier.");
uuid_id!(DiffId, "Semantic diff identifier.");
uuid_id!(ContractId, "Stable contract identity across versions.");
uuid_id!(ClaimId, "Individually addressed verification claim.");
uuid_id!(CertificateId, "Signed verification certificate identifier.");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nil_uuid() {
        assert_eq!(TenantId::from_uuid(Uuid::nil()), Err(IdError::NotUuidV7));
    }

    #[test]
    fn generates_v7() {
        assert_eq!(
            ObservationId::generate().as_uuid().get_version(),
            Some(Version::SortRand)
        );
    }
}
