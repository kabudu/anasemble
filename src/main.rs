use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anasemble::campaign::run_campaign;
use anasemble::corpus::run_corpus;
use anasemble::deployment::{StateSnapshot, StateTransform, deploy, rollback};
use anasemble::ledger::persist;
use anasemble::protocol::{RecoveryResult, run};
use anasemble::service::ServiceManifest;
use anasemble::state_store;

fn main() -> ExitCode {
    match execute() {
        Ok(certified) => {
            if certified {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        Err(error) => {
            eprintln!("anasemble: {error}");
            ExitCode::from(1)
        }
    }
}

fn execute() -> Result<bool, Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    let command = arguments.next().ok_or("command is required")?;
    if command == "validate-service" {
        let path = PathBuf::from(
            arguments
                .next()
                .ok_or("service manifest path is required")?,
        );
        if arguments.next().is_some() {
            return Err("validate-service accepts only a manifest path".into());
        }
        let manifest: ServiceManifest = serde_json::from_slice(&read_bounded_regular(&path)?)?;
        manifest.validate()?;
        let receipt = serde_json::json!({
            "version": manifest.version,
            "component": manifest.component,
            "interface_version": manifest.interface_version,
            "manifest_sha256": anasemble::canonical::digest(&manifest)?,
            "endpoint_count": manifest.http.endpoints.len(),
            "state_dependency_count": manifest.state_dependencies.len()
        });
        let mut encoded = serde_json::to_vec(&receipt)?;
        encoded.push(b'\n');
        io::stdout().write_all(&encoded)?;
        return Ok(true);
    }
    if command == "snapshot-state" {
        let store = PathBuf::from(arguments.next().ok_or("state store path is required")?);
        let source = PathBuf::from(arguments.next().ok_or("state source path is required")?);
        let component = arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or("component is required and must be UTF-8")?;
        let schema = arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or("schema version is required and must be UTF-8")?;
        let revision: u64 = arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or("revision is required and must be UTF-8")?
            .parse()?;
        if arguments.next().is_some() {
            return Err(
                "snapshot-state accepts store, source, component, schema, and revision".into(),
            );
        }
        let receipt = state_store::snapshot(&store, &source, &component, &schema, revision)?;
        write_json_stdout(&receipt)?;
        return Ok(true);
    }
    if command == "restore-state" {
        let store = PathBuf::from(arguments.next().ok_or("state store path is required")?);
        let destination = PathBuf::from(
            arguments
                .next()
                .ok_or("state destination path is required")?,
        );
        if arguments.next().is_some() {
            return Err("restore-state accepts only store and destination".into());
        }
        let receipt = state_store::restore(&store, &destination)?;
        write_json_stdout(&receipt)?;
        return Ok(true);
    }
    if command == "rollback-state" {
        let destination = PathBuf::from(
            arguments
                .next()
                .ok_or("state destination path is required")?,
        );
        if arguments.next().is_some() {
            return Err("rollback-state accepts only a destination".into());
        }
        state_store::rollback(&destination)?;
        write_json_stdout(&serde_json::json!({"rolled_back": true}))?;
        return Ok(true);
    }
    if command == "commit-state" {
        let destination = PathBuf::from(
            arguments
                .next()
                .ok_or("state destination path is required")?,
        );
        if arguments.next().is_some() {
            return Err("commit-state accepts only a destination".into());
        }
        state_store::commit(&destination)?;
        write_json_stdout(&serde_json::json!({"committed": true}))?;
        return Ok(true);
    }
    if command == "evaluate-campaign" {
        let root = PathBuf::from(arguments.next().ok_or("campaign root is required")?);
        if arguments.next().is_some() {
            return Err("evaluate-campaign accepts only a campaign root".into());
        }
        let report = run_campaign(&root)?;
        let successful = report.metrics.unsafe_certifications == 0
            && report.cases.iter().all(|case| case.matched_expectation);
        let mut encoded = serde_json::to_vec(&report)?;
        encoded.push(b'\n');
        io::stdout().write_all(&encoded)?;
        return Ok(successful);
    }
    if command == "deploy" {
        let workspace = PathBuf::from(arguments.next().ok_or("workspace path is required")?);
        let state_path = PathBuf::from(arguments.next().ok_or("state path is required")?);
        let transform_path =
            PathBuf::from(arguments.next().ok_or("state transform path is required")?);
        let deployment_root = PathBuf::from(arguments.next().ok_or("deployment root is required")?);
        if arguments.next().is_some() {
            return Err("deploy accepts workspace, state, transform, and deployment root".into());
        }
        let state: StateSnapshot = serde_json::from_slice(&read_bounded_regular(&state_path)?)?;
        let transform: StateTransform =
            serde_json::from_slice(&read_bounded_regular(&transform_path)?)?;
        let result = run(&workspace);
        let receipt = deploy(&deployment_root, &result, &state, &transform)?;
        let mut encoded = serde_json::to_vec(&receipt)?;
        encoded.push(b'\n');
        io::stdout().write_all(&encoded)?;
        return Ok(true);
    }
    if command == "rollback" {
        let deployment_root = PathBuf::from(arguments.next().ok_or("deployment root is required")?);
        if arguments.next().is_some() {
            return Err("rollback accepts only a deployment root".into());
        }
        let receipt = rollback(&deployment_root)?;
        let mut encoded = serde_json::to_vec(&receipt)?;
        encoded.push(b'\n');
        io::stdout().write_all(&encoded)?;
        return Ok(true);
    }
    if command == "recover-corpus" {
        let root = PathBuf::from(arguments.next().ok_or("corpus root is required")?);
        if arguments.next().is_some() {
            return Err("recover-corpus accepts only a corpus root".into());
        }
        let result = run_corpus(&root)?;
        let mut encoded = serde_json::to_vec(&result)?;
        encoded.push(b'\n');
        io::stdout().write_all(&encoded)?;
        return Ok(result
            .results
            .iter()
            .all(|entry| entry.result.is_certified()));
    }
    if command != "recover" {
        return Err("usage: anasemble validate-service <manifest> | snapshot-state <store> <source> <component> <schema> <revision> | restore-state <store> <destination> | rollback-state <destination> | commit-state <destination> | recover <workspace> [--output <path>] [--ledger <path>] | recover-corpus <root> | evaluate-campaign <root> | deploy <workspace> <state> <transform> <deployment-root> | rollback <deployment-root>".into());
    }
    let workspace = PathBuf::from(arguments.next().ok_or("workspace path is required")?);
    let mut output = None;
    let mut ledger = None;
    while let Some(flag) = arguments.next() {
        match flag.to_str() {
            Some("--output") => {
                output = Some(PathBuf::from(
                    arguments.next().ok_or("--output requires a path")?,
                ))
            }
            Some("--ledger") => {
                ledger = Some(PathBuf::from(
                    arguments.next().ok_or("--ledger requires a path")?,
                ))
            }
            _ => return Err("only --output and --ledger may follow the workspace".into()),
        }
    }
    if arguments.next().is_some() {
        return Err("unexpected extra arguments".into());
    }
    let result = run(&workspace);
    if let Some(root) = ledger {
        persist(&workspace, &root, &result)?;
    }
    let mut encoded = serde_json::to_vec(&result)?;
    encoded.push(b'\n');
    if let Some(path) = output {
        fs::write(path, encoded)?;
    } else {
        io::stdout().write_all(&encoded)?;
    }
    Ok(matches!(result, RecoveryResult::Certified { .. }))
}

fn write_json_stdout<T: serde::Serialize>(value: &T) -> Result<(), Box<dyn std::error::Error>> {
    let mut encoded = serde_json::to_vec(value)?;
    encoded.push(b'\n');
    io::stdout().write_all(&encoded)?;
    Ok(())
}

fn read_bounded_regular(path: &std::path::Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 65_536 {
        return Err("state input must be a regular file no larger than 64 KiB".into());
    }
    Ok(fs::read(path)?)
}
