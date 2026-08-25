//! Verifier traits, addressed claims, and signed certificates.
//!
//! A certificate may be stored by a consumer. An inference must not be upgraded
//! to verified without a certificate that independently checks.

#![forbid(unsafe_code)]

use anasemble_contract::ContractV1;
use anasemble_core::{BoundedText, Budget, CertificateId, ClaimId, Digest, encode_canonical_cbor};
use anasemble_evidence::{SignatureBlock, SigningKey};
use ed25519_dalek::{
    Signature, Signer, SigningKey as Ed25519SigningKey, Verifier as _, VerifyingKey,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Verification schema version.
pub const SCHEMA_VERSION: u16 = 1;

/// Fail-closed verification error.
#[derive(Debug, Error)]
pub enum VerificationError {
    #[error(transparent)]
    Core(#[from] anasemble_core::CoreError),
    #[error(transparent)]
    Id(#[from] anasemble_core::IdError),
    #[error(transparent)]
    Contract(#[from] anasemble_contract::ContractError),
    #[error("{0}")]
    Protocol(&'static str),
    #[error("{0}")]
    Crypto(&'static str),
    #[error("budget exhausted")]
    Budget,
}

/// Claim status. Proven requires evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    /// Evidence proves the claim.
    Proven,
    /// Evidence falsifies the claim.
    Falsified,
    /// Evidence is insufficient.
    Unknown,
    /// Claim does not apply to this candidate.
    NotApplicable,
}

/// One addressed claim.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerificationClaim {
    /// Claim identity.
    pub claim_id: ClaimId,
    /// Statement text.
    pub statement: BoundedText,
    /// Outcome.
    pub status: ClaimStatus,
    /// Evidence roots supporting the outcome.
    pub evidence: Vec<Digest>,
}

/// Evidence presented to a verifier.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EvidenceSet {
    /// Content digests the verifier may cite.
    pub roots: Vec<Digest>,
}

/// Verifier report. Truncation is not success for missing claims.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerificationReport {
    /// Schema version.
    pub schema_version: u16,
    /// Oracle identity.
    pub oracle_id: BoundedText,
    /// Oracle version.
    pub oracle_version: BoundedText,
    /// Addressed claims, sorted by claim id string.
    pub claims: Vec<VerificationClaim>,
    /// Evidence roots considered.
    pub evidence_roots: Vec<Digest>,
    /// Budget used.
    pub budget: Budget,
    /// True when the budget truncated evaluation.
    pub truncated: bool,
    /// Canonical digest of the unsigned report.
    pub digest: Digest,
}

#[derive(Serialize)]
struct UnsignedReport<'a> {
    schema_version: u16,
    oracle_id: &'a BoundedText,
    oracle_version: &'a BoundedText,
    claims: &'a [VerificationClaim],
    evidence_roots: &'a [Digest],
    budget: Budget,
    truncated: bool,
}

impl VerificationReport {
    /// Canonical digest.
    pub fn compute_digest(&self) -> Result<Digest, VerificationError> {
        Ok(Digest::sha256(&encode_canonical_cbor(&UnsignedReport {
            schema_version: self.schema_version,
            oracle_id: &self.oracle_id,
            oracle_version: &self.oracle_version,
            claims: &self.claims,
            evidence_roots: &self.evidence_roots,
            budget: self.budget,
            truncated: self.truncated,
        })?))
    }

    fn bind(mut self) -> Result<Self, VerificationError> {
        self.digest = self.compute_digest()?;
        Ok(self)
    }
}

/// Signed certificate over a report digest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CertificateV1 {
    /// Certificate identity.
    pub certificate_id: CertificateId,
    /// Report digest.
    pub report_digest: Digest,
    /// Ed25519 signature over the report digest bytes.
    pub signature: SignatureBlock,
}

/// Verify a contract against a candidate using cited evidence.
pub trait Verifier<C, X> {
    /// Evaluate claims. Budget exhaustion returns a truncated unknown report or an error.
    fn verify(
        &self,
        contract: &C,
        candidate: &X,
        evidence: &EvidenceSet,
        budget: Budget,
    ) -> Result<VerificationReport, VerificationError>;
}

/// Candidate digest presented for equality against a contract digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DigestCandidate {
    /// Digest of the candidate artefact.
    pub digest: Digest,
}

/// Proves or falsifies that a candidate digest matches the contract digest.
#[derive(Clone, Debug, Default)]
pub struct DigestEqualityVerifier {
    /// Oracle identity.
    pub oracle_id: String,
    /// Oracle version.
    pub oracle_version: String,
}

impl Verifier<ContractV1, DigestCandidate> for DigestEqualityVerifier {
    fn verify(
        &self,
        contract: &ContractV1,
        candidate: &DigestCandidate,
        evidence: &EvidenceSet,
        budget: Budget,
    ) -> Result<VerificationReport, VerificationError> {
        if budget.max_nodes() < 1 {
            return Err(VerificationError::Budget);
        }
        if contract.compute_digest()? != contract.digest {
            return Err(VerificationError::Protocol("contract digest mismatch"));
        }
        let status = if candidate.digest == contract.digest {
            ClaimStatus::Proven
        } else {
            ClaimStatus::Falsified
        };
        if status == ClaimStatus::Proven && evidence.roots.is_empty() {
            return Err(VerificationError::Protocol(
                "proven claim requires evidence roots",
            ));
        }
        let claim = VerificationClaim {
            claim_id: ClaimId::generate(),
            statement: BoundedText::new("candidate-digest-matches-contract")?,
            status,
            evidence: evidence.roots.clone(),
        };
        VerificationReport {
            schema_version: SCHEMA_VERSION,
            oracle_id: BoundedText::new(self.oracle_id.as_str())?,
            oracle_version: BoundedText::new(self.oracle_version.as_str())?,
            claims: vec![claim],
            evidence_roots: evidence.roots.clone(),
            budget,
            truncated: false,
            digest: Digest::sha256(b""),
        }
        .bind()
    }
}

/// Sign a report. Consumers must not treat an unsigned report as verified.
pub fn sign_report(
    report: &VerificationReport,
    signing: &SigningKey,
) -> Result<CertificateV1, VerificationError> {
    if report.compute_digest()? != report.digest {
        return Err(VerificationError::Protocol("report digest mismatch"));
    }
    if report.truncated {
        return Err(VerificationError::Protocol(
            "truncated report cannot be certified",
        ));
    }
    for claim in &report.claims {
        if claim.status == ClaimStatus::Proven && claim.evidence.is_empty() {
            return Err(VerificationError::Protocol(
                "proven claim requires evidence",
            ));
        }
    }
    let digest_bytes = report.digest.as_bytes();
    let signature = Ed25519SigningKey::from_bytes(signing.secret_bytes()).sign(&digest_bytes);
    Ok(CertificateV1 {
        certificate_id: CertificateId::generate(),
        report_digest: report.digest,
        signature: SignatureBlock {
            algorithm: BoundedText::new("ed25519")?,
            key_id: BoundedText::new(&signing.key_id)?,
            value: hex::encode(signature.to_bytes()),
        },
    })
}

/// Verify a certificate against a report and public key.
pub fn verify_certificate(
    report: &VerificationReport,
    certificate: &CertificateV1,
    public: &[u8; 32],
) -> Result<(), VerificationError> {
    if report.compute_digest()? != report.digest {
        return Err(VerificationError::Protocol("report digest mismatch"));
    }
    if certificate.report_digest != report.digest {
        return Err(VerificationError::Protocol("certificate report mismatch"));
    }
    if certificate.signature.algorithm.as_str() != "ed25519" {
        return Err(VerificationError::Crypto("unsupported signature algorithm"));
    }
    let bytes = hex::decode(&certificate.signature.value)
        .map_err(|_| VerificationError::Crypto("signature is not hex"))?;
    let bytes: [u8; 64] = bytes
        .try_into()
        .map_err(|_| VerificationError::Crypto("signature is not 64 bytes"))?;
    let digest_bytes = report.digest.as_bytes();
    VerifyingKey::from_bytes(public)
        .map_err(|_| VerificationError::Crypto("public key is invalid"))?
        .verify(&digest_bytes, &Signature::from_bytes(&bytes))
        .map_err(|_| VerificationError::Crypto("signature verification failed"))
}

/// Independent report digest.
pub fn independent_report_digest(report: &VerificationReport) -> Result<Digest, VerificationError> {
    report.compute_digest()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anasemble_core::CompatibilityClass;

    fn contract() -> ContractV1 {
        ContractV1::new("1.0.0", None, CompatibilityClass::Backward, Vec::new()).unwrap()
    }

    #[test]
    fn matching_digest_is_proven_only_with_evidence() {
        let contract = contract();
        let verifier = DigestEqualityVerifier {
            oracle_id: "digest-eq".into(),
            oracle_version: "1".into(),
        };
        let candidate = DigestCandidate {
            digest: contract.digest,
        };
        assert!(
            verifier
                .verify(
                    &contract,
                    &candidate,
                    &EvidenceSet::default(),
                    Budget::standard()
                )
                .is_err()
        );
        let evidence = EvidenceSet {
            roots: vec![Digest::sha256(b"obs")],
        };
        let report = verifier
            .verify(&contract, &candidate, &evidence, Budget::standard())
            .unwrap();
        assert_eq!(report.claims[0].status, ClaimStatus::Proven);
        assert_eq!(independent_report_digest(&report).unwrap(), report.digest);
        let signing = SigningKey::from_bytes("k1", [11_u8; 32]);
        let cert = sign_report(&report, &signing).unwrap();
        verify_certificate(&report, &cert, &signing.public_bytes()).unwrap();
    }

    #[test]
    fn mismatch_is_falsified() {
        let contract = contract();
        let verifier = DigestEqualityVerifier {
            oracle_id: "digest-eq".into(),
            oracle_version: "1".into(),
        };
        let report = verifier
            .verify(
                &contract,
                &DigestCandidate {
                    digest: Digest::sha256(b"other"),
                },
                &EvidenceSet {
                    roots: vec![Digest::sha256(b"obs")],
                },
                Budget::standard(),
            )
            .unwrap();
        assert_eq!(report.claims[0].status, ClaimStatus::Falsified);
    }
}
