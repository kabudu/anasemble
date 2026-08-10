use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::canonical::digest;
use crate::checker::{Coverage, certify};
use crate::checker_wire::encode_candidate as encode_checker_candidate;
use crate::fragments::{FragmentKind, IssuerPolicy, collect};
use crate::model::{Candidate, Error, FragmentContent, Grammar, RefusalCode};
use crate::oracle::{LossAttestation, attest_absence};
use crate::sandbox::{SandboxEvidence, compile as compile_wasm, verify as verify_wasm};
use crate::synthesizer::reconstruct;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    component: String,
    interface_version: String,
    grammar: Grammar,
    required_domains: usize,
    trusted_issuers: BTreeMap<String, IssuerPolicy>,
    loss_oracle: LossOraclePolicy,
    resource_limits: ResourceLimits,
    experiment: ExperimentRegistration,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LossOraclePolicy {
    forbidden_paths: Vec<PathBuf>,
    forbidden_sha256: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceLimits {
    max_fragments: usize,
    max_fragment_bytes: u64,
    max_workspace_files: u64,
    max_workspace_bytes: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExperimentRegistration {
    seed: u64,
    baselines: Vec<String>,
    primary_metrics: Vec<String>,
    secondary_metrics: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Certificate {
    protocol_version: &'static str,
    component: String,
    interface_version: String,
    survivor_envelope_digests: Vec<String>,
    normalized_constraints_digest: String,
    failure_domains: Vec<String>,
    grammar_version: String,
    search_bounds: SearchBounds,
    candidate_digest: String,
    non_identical_to_forbidden_artifacts: bool,
    checker_identity: &'static str,
    coverage: Coverage,
    sandbox: SandboxEvidence,
    state_transform: StateTransform,
    loss_attestation: LossAttestation,
    experiment: ExperimentRegistration,
    deployment_preconditions: [&'static str; 3],
}

#[derive(Debug, Serialize)]
struct SearchBounds {
    max_candidates: u64,
    examined: u64,
}

#[derive(Debug, Serialize)]
struct StateTransform {
    mode: &'static str,
    schema_digest: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "decision", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecoveryResult {
    Certified {
        candidate: Box<Candidate>,
        candidate_wasm_hex: String,
        certificate: Box<Certificate>,
    },
    Refused {
        code: RefusalCode,
        message: String,
    },
}

impl RecoveryResult {
    #[must_use]
    pub fn is_certified(&self) -> bool {
        matches!(self, Self::Certified { .. })
    }
}

pub fn run(workspace: &Path) -> RecoveryResult {
    match recover(workspace) {
        Ok(result) => result,
        Err(error) => RecoveryResult::Refused {
            code: error.refusal_code(),
            message: error.to_string(),
        },
    }
}

fn recover(workspace: &Path) -> Result<RecoveryResult, Error> {
    let registry_bytes = read_bounded(&workspace.join("registry.json"), 131_072)?;
    let registry: Registry = serde_json::from_slice(&registry_bytes)
        .map_err(|error| Error::InvalidRegistry(format!("registry JSON is invalid: {error}")))?;
    registry.grammar.validate()?;
    validate_registry(&registry)?;
    let fragment_directory = workspace.join("fragments");
    let mut fragment_paths =
        regular_json_files(&fragment_directory, registry.resource_limits.max_fragments)?;
    fragment_paths.sort();
    if fragment_paths.len() > registry.resource_limits.max_fragments {
        return Err(Error::SearchExhausted(
            "fragment-count bound exceeded".into(),
        ));
    }
    let mut envelopes = Vec::with_capacity(fragment_paths.len());
    for path in fragment_paths {
        let data = read_bounded(&path, registry.resource_limits.max_fragment_bytes)?;
        envelopes.push(serde_json::from_slice(&data).map_err(|error| {
            Error::InvalidEvidence(format!("fragment JSON is invalid: {error}"))
        })?);
    }
    let evidence = collect(
        envelopes,
        &registry.trusted_issuers,
        registry.required_domains,
        &registry.component,
        &registry.interface_version,
    )?;
    let attestation = attest_absence(
        workspace,
        &registry.loss_oracle.forbidden_paths,
        &registry.loss_oracle.forbidden_sha256,
        registry.resource_limits.max_workspace_files,
        registry.resource_limits.max_workspace_bytes,
    )?;

    let contents: Vec<_> = evidence
        .envelopes
        .iter()
        .map(|item| item.content.clone())
        .collect();
    let schemas: Vec<_> = evidence
        .envelopes
        .iter()
        .filter(|item| item.kind == FragmentKind::StateSchema)
        .map(|item| item.content.clone())
        .collect();
    let expected_schema = FragmentContent::StatePolicy {
        states: registry.grammar.states.clone(),
        initial_state: registry.grammar.initial_state.clone(),
    };
    if schemas != [expected_schema] {
        return Err(Error::InvalidEvidence(
            "state policy does not exactly match the grammar".into(),
        ));
    }
    let (candidate, examined) = reconstruct(
        &registry.component,
        &registry.interface_version,
        &registry.grammar,
        &contents,
    )?;
    let checker_candidate = encode_checker_candidate(&candidate)?;
    let candidate_digest = digest(&candidate)?;
    if registry
        .loss_oracle
        .forbidden_sha256
        .contains(&candidate_digest)
    {
        return Err(Error::CheckerRejected(
            "candidate is byte-identical to a forbidden artifact".into(),
        ));
    }
    let coverage = certify(
        &checker_candidate,
        &registry.component,
        &registry.interface_version,
        &contents,
    )?;
    let candidate_wasm = compile_wasm(&candidate)?;
    let sandbox = verify_wasm(&candidate, &candidate_wasm)?;
    let survivor_envelope_digests = evidence
        .envelopes
        .iter()
        .map(digest)
        .collect::<Result<Vec<_>, _>>()?;
    let certificate = Certificate {
        protocol_version: "regeneration-v0",
        component: registry.component.clone(),
        interface_version: registry.interface_version,
        survivor_envelope_digests,
        normalized_constraints_digest: digest(&contents)?,
        failure_domains: evidence.domains,
        grammar_version: registry.grammar.version.clone(),
        search_bounds: SearchBounds {
            max_candidates: registry.grammar.max_candidates,
            examined,
        },
        candidate_digest,
        non_identical_to_forbidden_artifacts: true,
        checker_identity: "anasemble.checker.separate-semantics-shared-serde-v0",
        coverage,
        sandbox,
        state_transform: StateTransform {
            mode: "identity",
            schema_digest: digest(&schemas[0])?,
        },
        loss_attestation: attestation,
        experiment: registry.experiment,
        deployment_preconditions: [
            "candidate digest unchanged",
            "atomic install",
            "rollback available",
        ],
    };
    Ok(RecoveryResult::Certified {
        candidate: Box::new(candidate),
        candidate_wasm_hex: hex::encode(candidate_wasm),
        certificate: Box::new(certificate),
    })
}

fn validate_registry(registry: &Registry) -> Result<(), Error> {
    if registry.component.is_empty() || registry.interface_version.is_empty() {
        return Err(Error::InvalidRegistry(
            "component and interface version are mandatory".into(),
        ));
    }
    if registry.required_domains == 0
        || registry.resource_limits.max_fragments == 0
        || registry.resource_limits.max_fragment_bytes == 0
        || registry.resource_limits.max_workspace_files == 0
        || registry.resource_limits.max_workspace_bytes == 0
    {
        return Err(Error::InvalidRegistry(
            "domain and resource bounds must be positive".into(),
        ));
    }
    if registry.required_domains > registry.trusted_issuers.len()
        || registry.resource_limits.max_fragments > 10_000
        || registry.resource_limits.max_fragment_bytes > 1_048_576
        || registry.resource_limits.max_workspace_files > 100_000
        || registry.resource_limits.max_workspace_bytes > 1_073_741_824
    {
        return Err(Error::InvalidRegistry(
            "configured recovery bounds exceed M0 safety maxima".into(),
        ));
    }
    if registry.experiment.baselines.is_empty()
        || registry.experiment.primary_metrics.is_empty()
        || registry.experiment.secondary_metrics.is_empty()
    {
        return Err(Error::InvalidRegistry(
            "experiment baselines and metrics are mandatory".into(),
        ));
    }
    if registry.loss_oracle.forbidden_paths.is_empty()
        || registry.loss_oracle.forbidden_sha256.is_empty()
        || registry
            .loss_oracle
            .forbidden_paths
            .iter()
            .any(|path| !path.is_absolute())
    {
        return Err(Error::InvalidRegistry(
            "loss oracle requires absolute paths and forbidden digests".into(),
        ));
    }
    if registry
        .loss_oracle
        .forbidden_sha256
        .iter()
        .any(|value| value.len() != 64 || hex::decode(value).is_err())
    {
        return Err(Error::InvalidRegistry(
            "forbidden artifact digests must be SHA-256 hex".into(),
        ));
    }
    for policy in registry.trusted_issuers.values() {
        if policy.failure_domain.is_empty()
            || hex::decode(&policy.hmac_sha256_key).map_or(true, |key| key.len() != 32)
        {
            return Err(Error::InvalidRegistry(
                "issuer policy requires a domain and 32-byte hex key".into(),
            ));
        }
    }
    Ok(())
}

fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(Error::InvalidEvidence(
            "JSON input is not a regular file".into(),
        ));
    }
    if metadata.len() > max_bytes {
        return Err(Error::SearchExhausted(
            "JSON input byte bound exceeded".into(),
        ));
    }
    Ok(fs::read(path)?)
}

fn regular_json_files(directory: &Path, max_files: usize) -> Result<Vec<PathBuf>, Error> {
    let mut paths = Vec::new();
    let mut entries = 0_usize;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        entries = entries
            .checked_add(1)
            .ok_or_else(|| Error::SearchExhausted("fragment entry counter overflow".into()))?;
        if entries > max_files {
            return Err(Error::SearchExhausted(
                "fragment-directory entry bound exceeded".into(),
            ));
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(Error::InvalidEvidence(
                "fragment directory contains a non-regular file".into(),
            ));
        }
        if entry
            .path()
            .extension()
            .is_some_and(|value| value == "json")
        {
            paths.push(entry.path());
        } else {
            return Err(Error::InvalidEvidence(
                "fragment directory contains a non-JSON file".into(),
            ));
        }
    }
    Ok(paths)
}
