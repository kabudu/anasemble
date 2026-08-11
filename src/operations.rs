//! Durable, bounded recovery-job operations for the local control plane.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::canonical::{bytes_digest, digest, encode};
use crate::model::{Error, RefusalCode};
use crate::protocol::{RecoveryResult, run};

const MAX_JOBS: usize = 1_024;
const MAX_RECORD_BYTES: u64 = 65_536;
const MAX_WORKSPACE_FILES: usize = 1_024;
const MAX_WORKSPACE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RESULT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OperationsConfig {
    pub version: String,
    pub max_queued: usize,
    pub max_batch: usize,
    pub max_attempts: u32,
    pub lease_seconds: u64,
}

impl Default for OperationsConfig {
    fn default() -> Self {
        Self {
            version: "operations-config-v1".into(),
            max_queued: 256,
            max_batch: 8,
            max_attempts: 3,
            lease_seconds: 300,
        }
    }
}

impl OperationsConfig {
    pub fn validate(&self) -> Result<(), Error> {
        if self.version != "operations-config-v1"
            || !(1..=MAX_JOBS).contains(&self.max_queued)
            || !(1..=64).contains(&self.max_batch)
            || !(1..=10).contains(&self.max_attempts)
            || !(10..=86_400).contains(&self.lease_seconds)
        {
            return Err(invalid(
                "operations configuration is outside supported bounds",
            ));
        }
        Ok(())
    }

    pub fn migrate(input: &[u8]) -> Result<Self, Error> {
        let value: serde_json::Value = serde_json::from_slice(input)?;
        match value.get("version").and_then(serde_json::Value::as_str) {
            Some("operations-config-v1") => {
                let config: Self = serde_json::from_value(value)?;
                config.validate()?;
                Ok(config)
            }
            Some("operations-config-v0") => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Legacy {
                    version: String,
                    queue_capacity: usize,
                    jobs_per_run: usize,
                    attempts: u32,
                    lease_seconds: u64,
                }
                let legacy: Legacy = serde_json::from_value(value)?;
                if legacy.version != "operations-config-v0" {
                    return Err(invalid("legacy operations configuration is invalid"));
                }
                let config = Self {
                    version: "operations-config-v1".into(),
                    max_queued: legacy.queue_capacity,
                    max_batch: legacy.jobs_per_run,
                    max_attempts: legacy.attempts,
                    lease_seconds: legacy.lease_seconds,
                };
                config.validate()?;
                Ok(config)
            }
            _ => Err(invalid("operations configuration version is unsupported")),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobState {
    Pending,
    Running,
    Certified,
    Refused,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JobEvent {
    pub sequence: u32,
    pub at_unix: u64,
    pub kind: String,
    pub previous_sha256: Option<String>,
    pub event_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecoveryJob {
    pub version: String,
    pub job_id: String,
    pub workspace: PathBuf,
    pub workspace_sha256: String,
    pub submitted_unix: u64,
    pub state: JobState,
    pub attempts: u32,
    pub lease_expires_unix: Option<u64>,
    pub result_sha256: Option<String>,
    pub refusal_code: Option<RefusalCode>,
    pub events: Vec<JobEvent>,
    pub record_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EnqueueReceipt {
    pub job_id: String,
    pub workspace_sha256: String,
    pub state: JobState,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OperationsMetrics {
    pub jobs_total: u64,
    pub pending: u64,
    pub running: u64,
    pub certified: u64,
    pub refused: u64,
    pub failed: u64,
    pub restart_recoveries: u64,
    pub attempts_total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OperationsStatus {
    pub version: String,
    pub healthy: bool,
    pub queue_capacity: usize,
    pub queue_available: usize,
    pub metrics: OperationsMetrics,
    pub diagnostic_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BatchReceipt {
    pub claimed: u64,
    pub certified: u64,
    pub refused: u64,
    pub failed: u64,
    pub recovered_after_restart: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SupportJob {
    pub job_id: String,
    pub workspace_sha256: String,
    pub state: JobState,
    pub attempts: u32,
    pub refusal_code: Option<RefusalCode>,
    pub event_digests: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SupportBundle {
    pub version: String,
    pub generated_unix: u64,
    pub config_sha256: String,
    pub status: OperationsStatus,
    pub jobs: Vec<SupportJob>,
    pub bundle_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PrunedJob {
    pub job_id: String,
    pub record_sha256: String,
    pub result_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PruneReceipt {
    pub version: String,
    pub jobs: Vec<PrunedJob>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunFailurePoint {
    None,
    AfterClaim,
}

pub struct OperationsStore {
    root: PathBuf,
    config: OperationsConfig,
}

impl OperationsStore {
    pub fn create(root: &Path, config: OperationsConfig) -> Result<Self, Error> {
        config.validate()?;
        if root.try_exists()? {
            return Err(invalid("operations root already exists"));
        }
        fs::create_dir(root)?;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        fs::create_dir(root.join("jobs"))?;
        fs::create_dir(root.join("results"))?;
        write_new(&root.join("config.json"), &encode(&config)?)?;
        File::open(root)?.sync_all()?;
        Ok(Self {
            root: root.into(),
            config,
        })
    }

    pub fn open(root: &Path) -> Result<Self, Error> {
        validate_directory(root)?;
        validate_directory(&root.join("jobs"))?;
        validate_directory(&root.join("results"))?;
        let bytes = read_regular(&root.join("config.json"), 65_536)?;
        let config = OperationsConfig::migrate(&bytes)?;
        Ok(Self {
            root: root.into(),
            config,
        })
    }

    pub fn enqueue(&self, workspace: &Path, submitted_unix: u64) -> Result<EnqueueReceipt, Error> {
        let workspace = workspace.canonicalize()?;
        validate_directory(&workspace)?;
        let workspace_sha256 = digest_workspace_reference(&workspace)?;
        let _lock = Lock::acquire(&self.root.join(".operations.lock"))?;
        let jobs = self.read_jobs()?;
        let queued = jobs
            .iter()
            .filter(|job| matches!(job.state, JobState::Pending | JobState::Running))
            .count();
        if queued >= self.config.max_queued {
            return Err(invalid("operations queue backpressure limit reached"));
        }
        let id_material = encode(&(workspace_sha256.as_str(), submitted_unix))?;
        let job_id = format!("job-{}", &bytes_digest(&id_material)[..24]);
        if self.job_path(&job_id).try_exists()? {
            return Err(invalid("duplicate recovery job already exists"));
        }
        let mut job = RecoveryJob {
            version: "recovery-job-v1".into(),
            job_id: job_id.clone(),
            workspace,
            workspace_sha256: workspace_sha256.clone(),
            submitted_unix,
            state: JobState::Pending,
            attempts: 0,
            lease_expires_unix: None,
            result_sha256: None,
            refusal_code: None,
            events: Vec::new(),
            record_sha256: String::new(),
        };
        append_event(&mut job, submitted_unix, "JOB_ENQUEUED")?;
        seal_record(&mut job)?;
        write_new(&self.job_path(&job_id), &encode(&job)?)?;
        File::open(self.root.join("jobs"))?.sync_all()?;
        Ok(EnqueueReceipt {
            job_id,
            workspace_sha256,
            state: JobState::Pending,
        })
    }

    pub fn run_recovery_batch(
        &self,
        now_unix: u64,
        failure: RunFailurePoint,
    ) -> Result<BatchReceipt, Error> {
        self.run_batch(now_unix, failure, run)
    }

    pub fn run_batch<F>(
        &self,
        now_unix: u64,
        failure: RunFailurePoint,
        mut executor: F,
    ) -> Result<BatchReceipt, Error>
    where
        F: FnMut(&Path) -> RecoveryResult,
    {
        let _runner = RunnerLease::acquire(&self.root, self.config.lease_seconds)?;
        let mut receipt = BatchReceipt {
            claimed: 0,
            certified: 0,
            refused: 0,
            failed: 0,
            recovered_after_restart: 0,
        };
        for _ in 0..self.config.max_batch {
            let Some((job, recovered)) = self.claim_next(now_unix)? else {
                break;
            };
            receipt.claimed += 1;
            receipt.recovered_after_restart += recovered;
            if failure == RunFailurePoint::AfterClaim {
                return Err(invalid("injected interruption after durable job claim"));
            }
            if digest_workspace_reference(&job.workspace)? != job.workspace_sha256 {
                self.finish_failed(&job.job_id, now_unix, "WORKSPACE_CHANGED_AFTER_ADMISSION")?;
                receipt.failed += 1;
                continue;
            }
            let result = executor(&job.workspace);
            let encoded = encode(&result)?;
            if encoded.len() > MAX_RESULT_BYTES {
                self.finish_failed(&job.job_id, now_unix, "RESULT_BOUND_EXCEEDED")?;
                receipt.failed += 1;
                continue;
            }
            let result_sha256 = bytes_digest(&encoded);
            self.write_result(&job.job_id, &encoded)?;
            match &result {
                RecoveryResult::Certified { .. } => {
                    self.finish(
                        &job.job_id,
                        now_unix,
                        JobState::Certified,
                        result_sha256,
                        None,
                    )?;
                    receipt.certified += 1;
                }
                RecoveryResult::Refused { code, .. } => {
                    self.finish(
                        &job.job_id,
                        now_unix,
                        JobState::Refused,
                        result_sha256,
                        Some(*code),
                    )?;
                    receipt.refused += 1;
                }
            }
        }
        Ok(receipt)
    }

    pub fn status(&self) -> Result<OperationsStatus, Error> {
        let _lock = Lock::acquire(&self.root.join(".operations.lock"))?;
        let jobs = self.read_jobs()?;
        let metrics = metrics(&jobs);
        let queued =
            usize::try_from(metrics.pending.saturating_add(metrics.running)).unwrap_or(usize::MAX);
        let mut diagnostic_codes = Vec::new();
        if queued >= self.config.max_queued {
            diagnostic_codes.push("QUEUE_SATURATED".into());
        }
        if metrics.failed > 0 {
            diagnostic_codes.push("FAILED_JOBS_PRESENT".into());
        }
        if metrics.running > 0 {
            diagnostic_codes.push("LEASED_JOBS_PRESENT".into());
        }
        Ok(OperationsStatus {
            version: "operations-status-v1".into(),
            healthy: diagnostic_codes.is_empty(),
            queue_capacity: self.config.max_queued,
            queue_available: self.config.max_queued.saturating_sub(queued),
            metrics,
            diagnostic_codes,
        })
    }

    pub fn result(&self, job_id: &str) -> Result<serde_json::Value, Error> {
        let _lock = Lock::acquire(&self.root.join(".operations.lock"))?;
        let job = self.read_job(job_id)?;
        if !matches!(job.state, JobState::Certified | JobState::Refused) {
            return Err(invalid("job does not have a terminal recovery result"));
        }
        let expected = job
            .result_sha256
            .ok_or_else(|| invalid("terminal job result digest is absent"))?;
        let path = self.root.join("results").join(format!("{job_id}.json"));
        let bytes = read_regular(&path, MAX_RESULT_BYTES as u64)?;
        if bytes_digest(&bytes) != expected {
            return Err(invalid("terminal job result digest is invalid"));
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn support_bundle(&self, generated_unix: u64) -> Result<SupportBundle, Error> {
        let _lock = Lock::acquire(&self.root.join(".operations.lock"))?;
        let jobs = self.read_jobs()?;
        let metrics = metrics(&jobs);
        let queued =
            usize::try_from(metrics.pending.saturating_add(metrics.running)).unwrap_or(usize::MAX);
        let mut diagnostic_codes = Vec::new();
        if queued >= self.config.max_queued {
            diagnostic_codes.push("QUEUE_SATURATED".into());
        }
        if metrics.failed > 0 {
            diagnostic_codes.push("FAILED_JOBS_PRESENT".into());
        }
        if metrics.running > 0 {
            diagnostic_codes.push("LEASED_JOBS_PRESENT".into());
        }
        let status = OperationsStatus {
            version: "operations-status-v1".into(),
            healthy: diagnostic_codes.is_empty(),
            queue_capacity: self.config.max_queued,
            queue_available: self.config.max_queued.saturating_sub(queued),
            metrics,
            diagnostic_codes,
        };
        let support_jobs = jobs
            .into_iter()
            .map(|job| SupportJob {
                job_id: job.job_id,
                workspace_sha256: job.workspace_sha256,
                state: job.state,
                attempts: job.attempts,
                refusal_code: job.refusal_code,
                event_digests: job
                    .events
                    .into_iter()
                    .map(|event| event.event_sha256)
                    .collect(),
            })
            .collect();
        let mut bundle = SupportBundle {
            version: "support-bundle-v1".into(),
            generated_unix,
            config_sha256: digest(&self.config)?,
            status,
            jobs: support_jobs,
            bundle_sha256: String::new(),
        };
        bundle.bundle_sha256 = digest(&bundle)?;
        Ok(bundle)
    }

    pub fn prune_terminal(
        &self,
        submitted_before_unix: u64,
        max_remove: usize,
    ) -> Result<PruneReceipt, Error> {
        if !(1..=256).contains(&max_remove) {
            return Err(invalid("prune limit must be between one and 256"));
        }
        let _lock = Lock::acquire(&self.root.join(".operations.lock"))?;
        let mut jobs: Vec<_> = self
            .read_jobs()?
            .into_iter()
            .filter(|job| {
                job.submitted_unix < submitted_before_unix
                    && matches!(
                        job.state,
                        JobState::Certified | JobState::Refused | JobState::Failed
                    )
            })
            .collect();
        jobs.sort_by_key(|job| (job.submitted_unix, job.job_id.clone()));
        let mut removed = Vec::new();
        for job in jobs.into_iter().take(max_remove) {
            let result = self
                .root
                .join("results")
                .join(format!("{}.json", job.job_id));
            if result.try_exists()? {
                let bytes = read_regular(&result, MAX_RESULT_BYTES as u64)?;
                let actual = bytes_digest(&bytes);
                if job.result_sha256.as_deref() != Some(actual.as_str()) {
                    return Err(invalid("pruned job result digest is inconsistent"));
                }
                fs::remove_file(&result)?;
            } else if job.result_sha256.is_some() {
                return Err(invalid("pruned job result is missing"));
            }
            fs::remove_file(self.job_path(&job.job_id))?;
            removed.push(PrunedJob {
                job_id: job.job_id,
                record_sha256: job.record_sha256,
                result_sha256: job.result_sha256,
            });
        }
        File::open(self.root.join("jobs"))?.sync_all()?;
        File::open(self.root.join("results"))?.sync_all()?;
        Ok(PruneReceipt {
            version: "prune-receipt-v1".into(),
            jobs: removed,
        })
    }

    fn claim_next(&self, now_unix: u64) -> Result<Option<(RecoveryJob, u64)>, Error> {
        let _lock = Lock::acquire(&self.root.join(".operations.lock"))?;
        let mut jobs = self.read_jobs()?;
        let mut recovered = 0;
        for job in &mut jobs {
            if job.state == JobState::Running
                && job
                    .lease_expires_unix
                    .is_some_and(|deadline| deadline <= now_unix)
            {
                if job.attempts >= self.config.max_attempts {
                    job.state = JobState::Failed;
                    job.lease_expires_unix = None;
                    append_event(job, now_unix, "RESTART_ATTEMPTS_EXHAUSTED")?;
                } else {
                    job.state = JobState::Pending;
                    job.lease_expires_unix = None;
                    append_event(job, now_unix, "JOB_RECOVERED_AFTER_RESTART")?;
                    recovered += 1;
                }
                self.replace_job(job)?;
            }
        }
        jobs.sort_by_key(|job| (job.submitted_unix, job.job_id.clone()));
        let Some(mut job) = jobs.into_iter().find(|job| job.state == JobState::Pending) else {
            return Ok(None);
        };
        job.state = JobState::Running;
        job.attempts += 1;
        job.lease_expires_unix = Some(now_unix.saturating_add(self.config.lease_seconds));
        append_event(&mut job, now_unix, "JOB_CLAIMED")?;
        self.replace_job(&mut job)?;
        Ok(Some((job, recovered)))
    }

    fn finish(
        &self,
        job_id: &str,
        now_unix: u64,
        state: JobState,
        result_sha256: String,
        refusal_code: Option<RefusalCode>,
    ) -> Result<(), Error> {
        let _lock = Lock::acquire(&self.root.join(".operations.lock"))?;
        let mut job = self.read_job(job_id)?;
        if job.state != JobState::Running {
            return Err(invalid("job completion lost its running lease"));
        }
        let event_kind = match state {
            JobState::Certified => "JOB_CERTIFIED",
            JobState::Refused => "JOB_REFUSED",
            _ => return Err(invalid("job completion state is invalid")),
        };
        job.state = state;
        job.lease_expires_unix = None;
        job.result_sha256 = Some(result_sha256);
        job.refusal_code = refusal_code;
        append_event(&mut job, now_unix, event_kind)?;
        self.replace_job(&mut job)
    }

    fn finish_failed(&self, job_id: &str, now_unix: u64, event: &str) -> Result<(), Error> {
        let _lock = Lock::acquire(&self.root.join(".operations.lock"))?;
        let mut job = self.read_job(job_id)?;
        job.state = JobState::Failed;
        job.lease_expires_unix = None;
        append_event(&mut job, now_unix, event)?;
        self.replace_job(&mut job)
    }

    fn write_result(&self, job_id: &str, bytes: &[u8]) -> Result<(), Error> {
        let path = self.root.join("results").join(format!("{job_id}.json"));
        if path.try_exists()? {
            let existing = read_regular(&path, MAX_RESULT_BYTES as u64)?;
            if existing != bytes {
                return Err(invalid("job result conflicts with durable result"));
            }
            return Ok(());
        }
        write_new(&path, bytes)
    }

    fn read_jobs(&self) -> Result<Vec<RecoveryJob>, Error> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(self.root.join("jobs"))? {
            if paths.len() >= MAX_JOBS {
                return Err(invalid("operations job count exceeded 4096"));
            }
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                return Err(invalid(
                    "operations jobs directory contains an unknown entry",
                ));
            }
            paths.push(path);
        }
        paths.sort();
        paths
            .iter()
            .map(|path| {
                let bytes = read_regular(path, MAX_RECORD_BYTES)?;
                let job: RecoveryJob = serde_json::from_slice(&bytes)?;
                validate_job(&job)?;
                Ok(job)
            })
            .collect()
    }

    fn read_job(&self, job_id: &str) -> Result<RecoveryJob, Error> {
        validate_job_id(job_id)?;
        let bytes = read_regular(&self.job_path(job_id), MAX_RECORD_BYTES)?;
        let job: RecoveryJob = serde_json::from_slice(&bytes)?;
        validate_job(&job)?;
        Ok(job)
    }

    fn replace_job(&self, job: &mut RecoveryJob) -> Result<(), Error> {
        seal_record(job)?;
        atomic_replace(
            &self.root.join("jobs"),
            &self.job_path(&job.job_id),
            &format!(".{}.tmp", job.job_id),
            &encode(job)?,
        )
    }

    fn job_path(&self, job_id: &str) -> PathBuf {
        self.root.join("jobs").join(format!("{job_id}.json"))
    }
}

fn append_event(job: &mut RecoveryJob, at_unix: u64, kind: &str) -> Result<(), Error> {
    if job.events.len() >= 64 || kind.len() > 64 {
        return Err(invalid("job audit event bound exceeded"));
    }
    let mut event = JobEvent {
        sequence: u32::try_from(job.events.len())
            .map_err(|_| invalid("event sequence overflow"))?,
        at_unix,
        kind: kind.into(),
        previous_sha256: job.events.last().map(|event| event.event_sha256.clone()),
        event_sha256: String::new(),
    };
    event.event_sha256 = digest(&event)?;
    job.events.push(event);
    Ok(())
}

fn seal_record(job: &mut RecoveryJob) -> Result<(), Error> {
    job.record_sha256.clear();
    job.record_sha256 = digest(job)?;
    Ok(())
}

fn validate_job(job: &RecoveryJob) -> Result<(), Error> {
    validate_job_id(&job.job_id)?;
    if job.version != "recovery-job-v1" || job.events.is_empty() || job.events.len() > 64 {
        return Err(invalid("recovery job structure is invalid"));
    }
    let mut previous = None;
    for (index, event) in job.events.iter().enumerate() {
        if event.sequence as usize != index || event.previous_sha256 != previous {
            return Err(invalid("recovery job audit chain is discontinuous"));
        }
        let mut unsigned = event.clone();
        let expected = std::mem::take(&mut unsigned.event_sha256);
        if digest(&unsigned)? != expected {
            return Err(invalid("recovery job audit event digest is invalid"));
        }
        previous = Some(expected);
    }
    let mut unsigned = job.clone();
    let expected = std::mem::take(&mut unsigned.record_sha256);
    if digest(&unsigned)? != expected {
        return Err(invalid("recovery job record digest is invalid"));
    }
    Ok(())
}

fn metrics(jobs: &[RecoveryJob]) -> OperationsMetrics {
    let mut metrics = OperationsMetrics::default();
    for job in jobs {
        metrics.jobs_total += 1;
        metrics.attempts_total += u64::from(job.attempts);
        metrics.restart_recoveries += job
            .events
            .iter()
            .filter(|event| event.kind == "JOB_RECOVERED_AFTER_RESTART")
            .count() as u64;
        match job.state {
            JobState::Pending => metrics.pending += 1,
            JobState::Running => metrics.running += 1,
            JobState::Certified => metrics.certified += 1,
            JobState::Refused => metrics.refused += 1,
            JobState::Failed => metrics.failed += 1,
        }
    }
    metrics
}

fn digest_workspace_reference(path: &Path) -> Result<String, Error> {
    let mut stack = vec![path.to_path_buf()];
    let mut files = BTreeMap::new();
    let mut total_bytes = 0_u64;
    while let Some(directory) = stack.pop() {
        validate_directory(&directory)?;
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let child = entry.path();
            let metadata = fs::symlink_metadata(&child)?;
            if metadata.file_type().is_symlink() {
                return Err(invalid("operations workspace contains a symbolic link"));
            }
            if metadata.is_dir() {
                stack.push(child);
                continue;
            }
            if !metadata.is_file() {
                return Err(invalid("operations workspace contains a non-regular entry"));
            }
            if files.len() >= MAX_WORKSPACE_FILES {
                return Err(invalid("operations workspace exceeds 1024 files"));
            }
            total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| invalid("operations workspace byte count overflow"))?;
            if total_bytes > MAX_WORKSPACE_BYTES {
                return Err(invalid("operations workspace exceeds 64 MiB"));
            }
            let relative = child
                .strip_prefix(path)
                .map_err(|_| invalid("operations workspace path escaped its root"))?
                .to_str()
                .ok_or_else(|| invalid("operations workspace path is not UTF-8"))?;
            files.insert(relative.to_owned(), bytes_digest(&fs::read(&child)?));
        }
    }
    digest(&files)
}

fn validate_job_id(value: &str) -> Result<(), Error> {
    if value.len() != 28
        || !value.starts_with("job-")
        || !value[4..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid("recovery job ID is invalid"));
    }
    Ok(())
}

fn validate_directory(path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid("operations path must be a real directory"));
    }
    Ok(())
}

fn read_regular(path: &Path, max: u64) -> Result<Vec<u8>, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > max {
        return Err(invalid(
            "operations file is invalid or exceeds its byte bound",
        ));
    }
    Ok(fs::read(path)?)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn atomic_replace(parent: &Path, path: &Path, temporary: &str, bytes: &[u8]) -> Result<(), Error> {
    let temporary = parent.join(temporary);
    if temporary.try_exists()? {
        return Err(invalid(
            "stale operations temporary file requires operator action",
        ));
    }
    write_new(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

struct Lock(PathBuf);
impl Lock {
    fn acquire(path: &Path) -> Result<Self, Error> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    invalid("operations store is locked")
                } else {
                    Error::Io(error)
                }
            })?
            .sync_all()?;
        Ok(Self(path.into()))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RunnerLeaseRecord {
    version: String,
    runner_id: String,
    expires_unix: u64,
}

struct RunnerLease {
    path: PathBuf,
    runner_id: String,
    stop: mpsc::Sender<()>,
    heartbeat: Option<thread::JoinHandle<()>>,
}

impl RunnerLease {
    fn acquire(root: &Path, lease_seconds: u64) -> Result<Self, Error> {
        let _lock = Lock::acquire(&root.join(".operations.lock"))?;
        let path = root.join(".runner-lease.json");
        let now_unix = system_unix()?;
        let runner_id = bytes_digest(
            format!("{}:{now_unix}:{}", std::process::id(), root.display()).as_bytes(),
        );
        let record = RunnerLeaseRecord {
            version: "runner-lease-v1".into(),
            runner_id: runner_id.clone(),
            expires_unix: now_unix.saturating_add(lease_seconds),
        };
        match write_new(&path, &encode(&record)?) {
            Ok(()) => Self::start(root, path, runner_id, lease_seconds),
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing: RunnerLeaseRecord =
                    serde_json::from_slice(&read_regular(&path, 16_384)?)?;
                if existing.version != "runner-lease-v1" || existing.expires_unix > now_unix {
                    return Err(invalid("another job runner owns the operations store"));
                }
                fs::remove_file(&path)?;
                write_new(&path, &encode(&record)?)?;
                Self::start(root, path, runner_id, lease_seconds)
            }
            Err(error) => Err(error),
        }
    }

    fn start(
        root: &Path,
        path: PathBuf,
        runner_id: String,
        lease_seconds: u64,
    ) -> Result<Self, Error> {
        let (stop, receiver) = mpsc::channel();
        let heartbeat_root = root.to_path_buf();
        let heartbeat_path = path.clone();
        let heartbeat_id = runner_id.clone();
        let interval = Duration::from_secs((lease_seconds / 3).max(1));
        let heartbeat = thread::Builder::new()
            .name("anasemble-runner-lease".into())
            .spawn(move || {
                while receiver.recv_timeout(interval).is_err() {
                    if renew_runner_lease(
                        &heartbeat_root,
                        &heartbeat_path,
                        &heartbeat_id,
                        lease_seconds,
                    )
                    .is_err()
                    {
                        break;
                    }
                }
            })
            .map_err(Error::Io)?;
        Ok(Self {
            path,
            runner_id,
            stop,
            heartbeat: Some(heartbeat),
        })
    }
}

impl Drop for RunnerLease {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(heartbeat) = self.heartbeat.take() {
            let _ = heartbeat.join();
        }
        let Ok(bytes) = read_regular(&self.path, 16_384) else {
            return;
        };
        let Ok(record) = serde_json::from_slice::<RunnerLeaseRecord>(&bytes) else {
            return;
        };
        if record.runner_id == self.runner_id {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn renew_runner_lease(
    root: &Path,
    path: &Path,
    runner_id: &str,
    lease_seconds: u64,
) -> Result<(), Error> {
    let _lock = Lock::acquire(&root.join(".operations.lock"))?;
    let mut record: RunnerLeaseRecord = serde_json::from_slice(&read_regular(path, 16_384)?)?;
    if record.runner_id != runner_id || record.version != "runner-lease-v1" {
        return Err(invalid("runner lease ownership changed"));
    }
    record.expires_unix = system_unix()?.saturating_add(lease_seconds);
    atomic_replace(root, path, ".runner-lease.tmp", &encode(&record)?)
}

fn system_unix() -> Result<u64, Error> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("system time is before the Unix epoch"))?
        .as_secs())
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn invalid(message: &str) -> Error {
    Error::InvalidEvidence(message.into())
}
