mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anasemble::lifecycle;
use anasemble::model::RefusalCode;
use anasemble::operations::{JobState, OperationsConfig, OperationsStore, RunFailurePoint};
use anasemble::protocol::RecoveryResult;
use tempfile::tempdir;

fn config(max_queued: usize, max_batch: usize) -> OperationsConfig {
    OperationsConfig {
        version: "operations-config-v1".into(),
        max_queued,
        max_batch,
        max_attempts: 3,
        lease_seconds: 10,
    }
}

#[test]
fn durable_jobs_recover_after_restart_apply_backpressure_and_redact_support() {
    let directory = tempdir().unwrap();
    let fixture = common::build_workspace(directory.path(), true);
    assert!(!fixture.artifact.exists());
    assert_eq!(fixture.artifact_digest.len(), 64);
    let root = directory.path().join("operations");
    let store = OperationsStore::create(&root, config(2, 1)).unwrap();
    let first = store.enqueue(&fixture.recovery, 100).unwrap();
    let second = store.enqueue(&fixture.recovery, 101).unwrap();
    assert_eq!(first.state, JobState::Pending);
    assert!(store.enqueue(&fixture.recovery, 102).is_err());

    assert!(
        store
            .run_batch(102, RunFailurePoint::AfterClaim, |_| unreachable!())
            .is_err()
    );
    let interrupted = store.status().unwrap();
    assert_eq!(interrupted.metrics.running, 1);
    assert_eq!(interrupted.metrics.pending, 1);
    assert!(
        interrupted
            .diagnostic_codes
            .contains(&"LEASED_JOBS_PRESENT".to_string())
    );

    let second_receipt = OperationsStore::open(&root)
        .unwrap()
        .run_batch(105, RunFailurePoint::None, |_| RecoveryResult::Refused {
            code: RefusalCode::InsufficientEvidence,
            message: "private diagnostic that must not enter support".into(),
        })
        .unwrap();
    assert_eq!(second_receipt.refused, 1);
    let recovered = OperationsStore::open(&root)
        .unwrap()
        .run_batch(113, RunFailurePoint::None, |_| RecoveryResult::Refused {
            code: RefusalCode::CheckerRejected,
            message: "second private diagnostic".into(),
        })
        .unwrap();
    assert_eq!(recovered.recovered_after_restart, 1);
    assert_eq!(recovered.refused, 1);

    let status = store.status().unwrap();
    assert!(status.healthy);
    assert_eq!(status.metrics.refused, 2);
    assert_eq!(status.metrics.restart_recoveries, 1);
    assert_eq!(status.metrics.attempts_total, 3);
    let bundle = store.support_bundle(120).unwrap();
    let encoded = serde_json::to_string(&bundle).unwrap();
    assert!(!encoded.contains(directory.path().to_str().unwrap()));
    assert!(!encoded.contains("private diagnostic"));
    assert!(encoded.contains(&first.job_id));
    assert!(encoded.contains(&second.job_id));

    store.enqueue(&fixture.recovery, 130).unwrap();
    let registry = fixture.recovery.join("registry.json");
    let mut changed = fs::read(&registry).unwrap();
    changed.push(b'\n');
    fs::write(&registry, changed).unwrap();
    let changed_receipt = store
        .run_batch(131, RunFailurePoint::None, |_| unreachable!())
        .unwrap();
    assert_eq!(changed_receipt.failed, 1);

    let record = root.join("jobs").join(format!("{}.json", first.job_id));
    let mut tampered: serde_json::Value =
        serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
    tampered["events"][0]["kind"] = "TAMPERED".into();
    fs::write(&record, serde_json::to_vec(&tampered).unwrap()).unwrap();
    assert!(store.status().is_err());
}

#[test]
fn runner_lease_serializes_workers_for_one_operations_store() {
    let directory = tempdir().unwrap();
    let fixture = common::build_workspace(directory.path(), true);
    let root = directory.path().join("operations");
    OperationsStore::create(&root, config(4, 1))
        .unwrap()
        .enqueue(&fixture.recovery, 100)
        .unwrap();
    let worker_root = root.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        OperationsStore::open(&worker_root)
            .unwrap()
            .run_batch(101, RunFailurePoint::None, |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                RecoveryResult::Refused {
                    code: RefusalCode::InsufficientEvidence,
                    message: "worker one".into(),
                }
            })
            .unwrap()
    });
    entered_rx.recv().unwrap();
    assert!(
        OperationsStore::open(&root)
            .unwrap()
            .run_batch(102, RunFailurePoint::None, |_| unreachable!())
            .is_err()
    );
    release_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap().refused, 1);
}

#[test]
fn bounded_scheduler_sustains_128_durable_jobs_without_growth() {
    let directory = tempdir().unwrap();
    let fixture = common::build_workspace(directory.path(), true);
    assert!(!fixture.artifact.exists());
    assert_eq!(fixture.artifact_digest.len(), 64);
    let root = directory.path().join("operations");
    let store = OperationsStore::create(&root, config(128, 64)).unwrap();
    let started = Instant::now();
    for sequence in 0..128 {
        store.enqueue(&fixture.recovery, 1_000 + sequence).unwrap();
    }
    assert!(store.enqueue(&fixture.recovery, 2_000).is_err());
    for batch in 0..2 {
        let receipt = store
            .run_batch(2_100 + batch, RunFailurePoint::None, |_| {
                RecoveryResult::Refused {
                    code: RefusalCode::InsufficientEvidence,
                    message: "bounded performance fixture".into(),
                }
            })
            .unwrap();
        assert_eq!(receipt.claimed, 64);
    }
    let status = store.status().unwrap();
    assert_eq!(status.metrics.jobs_total, 128);
    assert_eq!(status.metrics.refused, 128);
    assert_eq!(status.queue_available, 128);
    assert!(started.elapsed() < Duration::from_secs(20));
    let pruned = store.prune_terminal(2_000, 64).unwrap();
    assert_eq!(pruned.jobs.len(), 64);
    assert_eq!(store.status().unwrap().metrics.jobs_total, 64);
}

#[test]
fn operations_retry_transient_lock_contention_but_refuse_a_persistent_lock() {
    let directory = tempdir().unwrap();
    let root = directory.path().join("operations");
    let store = OperationsStore::create(&root, config(8, 4)).unwrap();
    let lock = root.join(".operations.lock");
    fs::write(&lock, b"transient").unwrap();
    let transient = lock.clone();
    let remover = thread::spawn(move || {
        thread::sleep(Duration::from_millis(25));
        fs::remove_file(transient).unwrap();
    });
    assert!(store.status().is_ok());
    remover.join().unwrap();

    fs::write(&lock, b"persistent").unwrap();
    assert!(store.status().is_err());
    fs::remove_file(lock).unwrap();
}

#[test]
fn public_cli_runs_recovery_reports_status_migrates_config_and_uninstalls_exactly() {
    let directory = tempdir().unwrap();
    let fixture = common::build_workspace(&directory.path().join("fixture"), true);
    assert!(!fixture.artifact.exists());
    assert_eq!(fixture.artifact_digest.len(), 64);
    let binary = env!("CARGO_BIN_EXE_anasemble");
    let legacy = directory.path().join("legacy.json");
    common::write_json(
        &legacy,
        &serde_json::json!({
            "version":"operations-config-v0",
            "queue_capacity":8,
            "jobs_per_run":2,
            "attempts":3,
            "lease_seconds":30
        }),
    );
    let migrated = directory.path().join("migrated.json");
    command(
        binary,
        &["migrate-operations-config", path(&legacy), path(&migrated)],
    );
    let root = directory.path().join("operations");
    command(binary, &["init-operations", path(&root), path(&migrated)]);
    let enqueued = command(
        binary,
        &[
            "enqueue-recovery",
            path(&root),
            path(&fixture.recovery),
            "100",
        ],
    );
    let run = command(binary, &["run-jobs", path(&root), "101"]);
    assert_eq!(run["certified"], 1);
    let status = command(binary, &["operations-status", path(&root)]);
    assert_eq!(status["metrics"]["certified"], 1);
    let result = command(
        binary,
        &[
            "job-result",
            path(&root),
            enqueued["job_id"].as_str().unwrap(),
        ],
    );
    assert_eq!(result["decision"], "CERTIFIED");
    let support = directory.path().join("support.json");
    command(
        binary,
        &["create-support-bundle", path(&root), "102", path(&support)],
    );
    let support_bytes = fs::read_to_string(&support).unwrap();
    assert!(!support_bytes.contains(fixture.recovery.to_str().unwrap()));
    assert_eq!(
        fs::metadata(&support).unwrap().permissions().mode() & 0o077,
        0
    );

    let prefix = directory.path().join("installed");
    let installed = command(binary, &["install", path(&prefix)]);
    assert_eq!(installed["installed_files"], 4);
    assert!(prefix.join("bin/anasemble").exists());
    let extra = prefix.join("share/extra");
    fs::write(&extra, b"operator-owned").unwrap();
    assert!(lifecycle::uninstall(&prefix).is_err());
    assert!(prefix.join("bin/anasemble").exists());
    fs::remove_file(extra).unwrap();
    let compatibility = prefix.join("share/compatibility-v2.json");
    let matrix: lifecycle::CompatibilityManifest =
        serde_json::from_slice(&fs::read(&compatibility).unwrap()).unwrap();
    assert_eq!(matrix.version, "compatibility-v2");
    let mut profile_ids = std::collections::BTreeSet::new();
    for profile in &matrix.profiles {
        assert!(profile_ids.insert(&profile.id), "duplicate profile id");
        for evidence in &profile.evidence {
            assert!(
                Path::new(evidence).is_file(),
                "missing evidence: {evidence}"
            );
        }
        if profile.support == lifecycle::SupportStatus::Supported {
            assert_eq!(
                profile.implementation,
                lifecycle::ImplementationStatus::Implemented
            );
            assert_eq!(profile.validation, lifecycle::ValidationStatus::Tested);
            assert!(!profile.evidence.is_empty());
        }
    }
    assert!(matrix.profiles.iter().any(|profile| {
        profile.id == "macos-arm64-p2-local-state"
            && profile.validation == lifecycle::ValidationStatus::Tested
            && profile.support == lifecycle::SupportStatus::Supported
            && !profile.evidence.is_empty()
    }));
    assert!(matrix.profiles.iter().any(|profile| {
        profile.id == "aws-managed-state-eu-west-1"
            && profile.implementation == lifecycle::ImplementationStatus::Implemented
            && profile.validation == lifecycle::ValidationStatus::Tested
            && profile.support == lifecycle::SupportStatus::Supported
    }));
    let original = fs::read(&compatibility).unwrap();
    fs::write(&compatibility, b"changed").unwrap();
    assert!(lifecycle::uninstall(&prefix).is_err());
    fs::write(&compatibility, original).unwrap();
    assert!(lifecycle::uninstall(&prefix).unwrap().removed_prefix);
    assert!(!prefix.exists());
}

fn command(binary: &str, arguments: &[&str]) -> serde_json::Value {
    let output = Command::new(binary).args(arguments).output().unwrap();
    assert!(
        output.status.success(),
        "{:?}: {}",
        arguments,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn path(value: &std::path::Path) -> &str {
    value.to_str().unwrap()
}
