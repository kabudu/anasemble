use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anasemble::corpus::run_corpus;
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
        return Err("usage: anasemble recover <workspace> [--output <path>] [--ledger <path>] | anasemble recover-corpus <root>".into());
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
