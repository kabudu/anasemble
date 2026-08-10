mod common;

use std::process::Command;

use common::build_workspace;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn fresh_process_is_deterministic_and_receives_only_workspace() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    let execute = || {
        Command::new(env!("CARGO_BIN_EXE_anasemble"))
            .arg("recover")
            .arg(&workspace.recovery)
            .current_dir(&workspace.recovery)
            .env_clear()
            .env("LC_ALL", "C")
            .output()
            .unwrap()
    };
    let first = execute();
    let second = execute();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(first.stdout, second.stdout);
    let output: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(output["decision"], "CERTIFIED");
    assert!(!workspace.artifact.exists());
    assert!(!String::from_utf8_lossy(&first.stdout).contains(&workspace.artifact_digest));
}
