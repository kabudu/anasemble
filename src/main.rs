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
        return Err("usage: anasemble recover <workspace> [--output <path>] [--ledger <path>] | recover-corpus <root> | evaluate-campaign <root> | deploy <workspace> <state> <transform> <deployment-root> | rollback <deployment-root>".into());
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

fn read_bounded_regular(path: &std::path::Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 65_536 {
        return Err("state input must be a regular file no larger than 64 KiB".into());
    }
    Ok(fs::read(path)?)
}
