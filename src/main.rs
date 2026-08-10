use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

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
    if arguments.next().as_deref() != Some("recover".as_ref()) {
        return Err("usage: anasemble recover <workspace> [--output <path>]".into());
    }
    let workspace = PathBuf::from(arguments.next().ok_or("workspace path is required")?);
    let mut output = None;
    if let Some(flag) = arguments.next() {
        if flag != "--output" {
            return Err("only --output may follow the workspace".into());
        }
        output = Some(PathBuf::from(
            arguments.next().ok_or("--output requires a path")?,
        ));
    }
    if arguments.next().is_some() {
        return Err("unexpected extra arguments".into());
    }
    let result = run(&workspace);
    let mut encoded = serde_json::to_vec(&result)?;
    encoded.push(b'\n');
    if let Some(path) = output {
        fs::write(path, encoded)?;
    } else {
        io::stdout().write_all(&encoded)?;
    }
    Ok(matches!(result, RecoveryResult::Certified { .. }))
}
