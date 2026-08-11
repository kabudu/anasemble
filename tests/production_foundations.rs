mod common;

use std::fs;
use std::process::Command;

use anasemble::state_store::{self, RestoreFailurePoint};
use serde_json::{Value, json};
use tempfile::tempdir;

use common::{build_workspace, write_json};

fn service_manifest() -> Value {
    json!({
        "version": "service-v1",
        "component": "turnstile",
        "interface_version": "1",
        "http": {
            "endpoints": [{
                "method": "POST",
                "path": "/transition",
                "request_schema_sha256": "11".repeat(32),
                "response_schema_sha256": "22".repeat(32)
            }]
        },
        "effects": [{"kind": "state", "target": "turnstile-state", "access": "read_write"}],
        "state_dependencies": [{
            "name": "turnstile-state",
            "adapter": "filesystem",
            "consistency": "snapshot",
            "required": true
        }],
        "limits": {
            "request_bytes": 4096,
            "response_bytes": 4096,
            "wall_time_ms": 1000,
            "concurrent_requests": 8
        }
    })
}

#[test]
fn service_manifest_is_validated_by_cli_and_bound_to_certificate() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    assert!(!workspace.artifact.exists());
    assert_eq!(workspace.artifact_digest.len(), 64);
    let path = directory.path().join("service.json");
    write_json(&path, &service_manifest());
    let validation = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .args(["validate-service"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        validation.status.success(),
        "{}",
        String::from_utf8_lossy(&validation.stderr)
    );
    let receipt: Value = serde_json::from_slice(&validation.stdout).unwrap();

    let registry_path = workspace.recovery.join("registry.json");
    let mut registry: Value = serde_json::from_slice(&fs::read(&registry_path).unwrap()).unwrap();
    registry["service_manifest"] = service_manifest();
    write_json(&registry_path, &registry);
    let recovery = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .args(["recover"])
        .arg(&workspace.recovery)
        .output()
        .unwrap();
    assert!(
        recovery.status.success(),
        "{}",
        String::from_utf8_lossy(&recovery.stderr)
    );
    let result: Value = serde_json::from_slice(&recovery.stdout).unwrap();
    assert_eq!(
        result["certificate"]["service_manifest_digest"],
        receipt["manifest_sha256"]
    );
}

#[test]
fn mismatched_service_identity_refuses_before_recovery() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    assert!(!workspace.artifact.exists());
    assert_eq!(workspace.artifact_digest.len(), 64);
    let registry_path = workspace.recovery.join("registry.json");
    let mut registry: Value = serde_json::from_slice(&fs::read(&registry_path).unwrap()).unwrap();
    let mut manifest = service_manifest();
    manifest["component"] = Value::String("other-service".into());
    registry["service_manifest"] = manifest;
    write_json(&registry_path, &registry);
    let recovery = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .args(["recover"])
        .arg(&workspace.recovery)
        .output()
        .unwrap();
    assert_eq!(recovery.status.code(), Some(2));
    let result: Value = serde_json::from_slice(&recovery.stdout).unwrap();
    assert_eq!(result["code"], "INVALID_REGISTRY");
}

#[test]
fn filesystem_state_round_trip_supports_commit_and_rollback() {
    let directory = tempdir().unwrap();
    let store = directory.path().join("store");
    let source = directory.path().join("source.db");
    let destination = directory.path().join("active.db");
    fs::write(&source, b"recovered-state").unwrap();
    fs::write(&destination, b"previous-state").unwrap();
    state_store::snapshot(&store, &source, "orders", "schema-2", 42).unwrap();
    let receipt = state_store::restore(&store, &destination).unwrap();
    assert!(receipt.rollback_available);
    assert_eq!(fs::read(&destination).unwrap(), b"recovered-state");
    state_store::rollback(&destination).unwrap();
    assert_eq!(fs::read(&destination).unwrap(), b"previous-state");

    state_store::restore(&store, &destination).unwrap();
    state_store::commit(&destination).unwrap();
    assert_eq!(fs::read(&destination).unwrap(), b"recovered-state");
    assert!(
        !directory
            .path()
            .join("active.db.anasemble-rollback")
            .exists()
    );
    assert!(
        !directory
            .path()
            .join("active.db.anasemble-activation.json")
            .exists()
    );
}

#[test]
fn commit_refuses_to_discard_rollback_after_active_state_changes() {
    let directory = tempdir().unwrap();
    let store = directory.path().join("store");
    let source = directory.path().join("source.db");
    let destination = directory.path().join("active.db");
    fs::write(&source, b"replacement").unwrap();
    fs::write(&destination, b"original").unwrap();
    state_store::snapshot(&store, &source, "orders", "schema-1", 2).unwrap();
    state_store::restore(&store, &destination).unwrap();
    fs::write(&destination, b"unexpected-change").unwrap();
    assert!(state_store::commit(&destination).is_err());
    assert!(
        directory
            .path()
            .join("active.db.anasemble-rollback")
            .exists()
    );
}

#[test]
fn corruption_lock_and_stale_files_fail_closed() {
    let directory = tempdir().unwrap();
    let store = directory.path().join("store");
    let source = directory.path().join("source.db");
    let destination = directory.path().join("active.db");
    fs::write(&source, b"state").unwrap();
    let receipt = state_store::snapshot(&store, &source, "orders", "schema-1", 1).unwrap();
    fs::write(
        store
            .join("objects")
            .join(format!("{}.bin", receipt.payload_sha256)),
        b"tampered",
    )
    .unwrap();
    assert!(state_store::restore(&store, &destination).is_err());

    fs::write(store.join(".snapshot.lock"), b"").unwrap();
    assert!(state_store::restore(&store, &destination).is_err());
    fs::remove_file(store.join(".snapshot.lock")).unwrap();

    fs::write(&source, b"state-2").unwrap();
    state_store::snapshot(&store, &source, "orders", "schema-1", 2).unwrap();
    fs::write(
        directory.path().join(".active.db.anasemble-stage"),
        b"stale",
    )
    .unwrap();
    assert!(state_store::restore(&store, &destination).is_err());
}

#[test]
fn snapshot_rejects_replayed_revision_and_component_change() {
    let directory = tempdir().unwrap();
    let store = directory.path().join("store");
    let source = directory.path().join("source.db");
    fs::write(&source, b"state").unwrap();
    state_store::snapshot(&store, &source, "orders", "schema-1", 7).unwrap();
    assert!(state_store::snapshot(&store, &source, "orders", "schema-1", 7).is_err());
    assert!(state_store::snapshot(&store, &source, "other", "schema-1", 8).is_err());
}

#[test]
fn injected_restore_failure_preserves_previous_state() {
    let directory = tempdir().unwrap();
    let store = directory.path().join("store");
    let source = directory.path().join("source.db");
    let destination = directory.path().join("active.db");
    fs::write(&source, b"replacement").unwrap();
    fs::write(&destination, b"original").unwrap();
    state_store::snapshot(&store, &source, "orders", "schema-1", 2).unwrap();
    assert!(
        state_store::restore_with_failure(
            &store,
            &destination,
            RestoreFailurePoint::AfterRollbackPrepared,
        )
        .is_err()
    );
    assert_eq!(fs::read(&destination).unwrap(), b"original");
    assert!(
        !directory
            .path()
            .join("active.db.anasemble-rollback")
            .exists()
    );
    assert!(!directory.path().join(".active.db.anasemble-stage").exists());
}

#[test]
fn public_state_cli_executes_snapshot_restore_and_rollback() {
    let directory = tempdir().unwrap();
    let store = directory.path().join("store");
    let source = directory.path().join("source.db");
    let destination = directory.path().join("active.db");
    fs::write(&source, b"replacement").unwrap();
    fs::write(&destination, b"original").unwrap();
    let run = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_anasemble"))
            .args(arguments)
            .output()
            .unwrap()
    };
    let snapshot = run(&[
        "snapshot-state",
        store.to_str().unwrap(),
        source.to_str().unwrap(),
        "orders",
        "schema-1",
        "7",
    ]);
    assert!(
        snapshot.status.success(),
        "{}",
        String::from_utf8_lossy(&snapshot.stderr)
    );
    let restore = run(&[
        "restore-state",
        store.to_str().unwrap(),
        destination.to_str().unwrap(),
    ]);
    assert!(
        restore.status.success(),
        "{}",
        String::from_utf8_lossy(&restore.stderr)
    );
    assert_eq!(fs::read(&destination).unwrap(), b"replacement");
    let rollback = run(&["rollback-state", destination.to_str().unwrap()]);
    assert!(
        rollback.status.success(),
        "{}",
        String::from_utf8_lossy(&rollback.stderr)
    );
    assert_eq!(fs::read(&destination).unwrap(), b"original");
}
