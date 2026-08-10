use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::canonical::{bytes_digest, encode};
use crate::model::{Candidate, Error};
use crate::protocol::RecoveryResult;

const MAX_DEPLOYMENT_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StateSnapshot {
    pub schema_version: String,
    pub state: String,
    pub revision: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StateTransform {
    pub from_schema: String,
    pub to_schema: String,
    pub state_map: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentBundle {
    pub version: String,
    pub candidate: Candidate,
    pub candidate_digest: String,
    pub state: StateSnapshot,
    pub source_state_digest: String,
    pub state_transform_digest: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailurePoint {
    None,
    AfterRollbackPrepared,
    BeforeActivation,
}

#[derive(Debug, Serialize)]
pub struct DeploymentReceipt {
    pub active_digest: String,
    pub candidate_digest: String,
    pub state_transform_digest: String,
    pub rollback_available: bool,
    pub state_revision: u64,
}

pub fn deploy(
    root: &Path,
    recovery: &RecoveryResult,
    source: &StateSnapshot,
    transform: &StateTransform,
) -> Result<DeploymentReceipt, Error> {
    deploy_with_failure(root, recovery, source, transform, FailurePoint::None)
}

pub fn deploy_with_failure(
    root: &Path,
    recovery: &RecoveryResult,
    source: &StateSnapshot,
    transform: &StateTransform,
    failure: FailurePoint,
) -> Result<DeploymentReceipt, Error> {
    let RecoveryResult::Certified { candidate, .. } = recovery else {
        return Err(Error::CheckerRejected(
            "only a certified recovery can be deployed".into(),
        ));
    };
    validate_transform(candidate, source, transform)?;
    fs::create_dir_all(root)?;
    reject_links(root)?;
    let _lock = DeploymentLock::acquire(root)?;
    let active = root.join("active.json");
    let rollback = root.join("rollback.json");
    let had_active = active.try_exists()?;
    if had_active {
        let previous = read_regular_bounded(&active)?;
        decode_bundle(&previous)?;
        atomic_replace(root, &rollback, "rollback.tmp", &previous)?;
    }
    if failure == FailurePoint::AfterRollbackPrepared {
        return Err(Error::Io(std::io::Error::other(
            "injected failure after rollback preparation",
        )));
    }
    let mapped = transform.state_map.get(&source.state).ok_or_else(|| {
        Error::InvalidEvidence("state transform has no mapping for current state".into())
    })?;
    let revision = source
        .revision
        .checked_add(1)
        .ok_or_else(|| Error::InvalidEvidence("state revision overflow".into()))?;
    let bundle = DeploymentBundle {
        version: "deployment-v1".into(),
        candidate: (**candidate).clone(),
        candidate_digest: crate::canonical::digest(candidate.as_ref())?,
        state: StateSnapshot {
            schema_version: transform.to_schema.clone(),
            state: mapped.clone(),
            revision,
        },
        source_state_digest: crate::canonical::digest(source)?,
        state_transform_digest: crate::canonical::digest(transform)?,
    };
    let bytes = encode(&bundle)?;
    if bytes.len() > MAX_DEPLOYMENT_BYTES {
        return Err(Error::SearchExhausted(
            "deployment bundle exceeds 1 MiB".into(),
        ));
    }
    let staged = root.join("active.tmp");
    write_new_synced(&staged, &bytes)?;
    if failure == FailurePoint::BeforeActivation {
        fs::remove_file(staged)?;
        return Err(Error::Io(std::io::Error::other(
            "injected failure before activation",
        )));
    }
    fs::rename(&staged, &active)?;
    File::open(root)?.sync_all()?;
    Ok(DeploymentReceipt {
        active_digest: bytes_digest(&bytes),
        candidate_digest: crate::canonical::digest(candidate.as_ref())?,
        state_transform_digest: bundle.state_transform_digest,
        rollback_available: had_active,
        state_revision: revision,
    })
}

pub fn rollback(root: &Path) -> Result<DeploymentReceipt, Error> {
    reject_links(root)?;
    let _lock = DeploymentLock::acquire(root)?;
    let rollback_path = root.join("rollback.json");
    let bytes = read_regular_bounded(&rollback_path)?;
    let bundle = decode_bundle(&bytes)?;
    atomic_replace(root, &root.join("active.json"), "active.tmp", &bytes)?;
    Ok(DeploymentReceipt {
        active_digest: bytes_digest(&bytes),
        candidate_digest: crate::canonical::digest(&bundle.candidate)?,
        state_transform_digest: bundle.state_transform_digest,
        rollback_available: true,
        state_revision: bundle.state.revision,
    })
}

pub fn read_active(root: &Path) -> Result<DeploymentBundle, Error> {
    let bytes = read_regular_bounded(&root.join("active.json"))?;
    decode_bundle(&bytes)
}

fn decode_bundle(bytes: &[u8]) -> Result<DeploymentBundle, Error> {
    let bundle: DeploymentBundle = serde_json::from_slice(bytes)
        .map_err(|error| Error::InvalidEvidence(format!("deployment image is invalid: {error}")))?;
    bundle.candidate.grammar.validate()?;
    if bundle.version != "deployment-v1"
        || bundle.candidate_digest != crate::canonical::digest(&bundle.candidate)?
        || !bundle
            .candidate
            .grammar
            .states
            .contains(&bundle.state.state)
        || bundle.state.schema_version != bundle.candidate.grammar.version
        || !is_digest(&bundle.source_state_digest)
        || !is_digest(&bundle.state_transform_digest)
    {
        return Err(Error::InvalidEvidence(
            "deployment image integrity or state binding is invalid".into(),
        ));
    }
    Ok(bundle)
}

fn is_digest(value: &str) -> bool {
    value.len() == 64 && hex::decode(value).is_ok()
}

fn validate_transform(
    candidate: &Candidate,
    source: &StateSnapshot,
    transform: &StateTransform,
) -> Result<(), Error> {
    if transform.from_schema != source.schema_version
        || transform.to_schema != candidate.grammar.version
        || transform.state_map.is_empty()
        || transform.state_map.len() > 64
        || transform
            .state_map
            .values()
            .any(|state| !candidate.grammar.states.contains(state))
    {
        return Err(Error::InvalidEvidence(
            "state transform does not match the source and candidate schemas".into(),
        ));
    }
    Ok(())
}

fn atomic_replace(
    root: &Path,
    target: &Path,
    temporary_name: &str,
    data: &[u8],
) -> Result<(), Error> {
    let temporary = root.join(temporary_name);
    if temporary.try_exists()? {
        return Err(Error::InvalidEvidence(
            "stale deployment staging file".into(),
        ));
    }
    write_new_synced(&temporary, data)?;
    fs::rename(&temporary, target)?;
    File::open(root)?.sync_all()?;
    Ok(())
}

fn write_new_synced(path: &Path, data: &[u8]) -> Result<(), Error> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(())
}

fn read_regular_bounded(path: &Path) -> Result<Vec<u8>, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_DEPLOYMENT_BYTES as u64
    {
        return Err(Error::InvalidEvidence(
            "deployment image is not a bounded regular file".into(),
        ));
    }
    Ok(fs::read(path)?)
}

fn reject_links(root: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::InvalidEvidence(
            "deployment root must be a real directory".into(),
        ));
    }
    Ok(())
}

struct DeploymentLock {
    path: std::path::PathBuf,
    _file: File,
}

impl DeploymentLock {
    fn acquire(root: &Path) -> Result<Self, Error> {
        let path = root.join(".deploy.lock");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                Error::Io(std::io::Error::new(
                    error.kind(),
                    "deployment transaction is locked",
                ))
            })?;
        Ok(Self { path, _file: file })
    }
}

impl Drop for DeploymentLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
