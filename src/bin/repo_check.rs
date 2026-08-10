use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

const REQUIRED: &[&str] = &[
    "AGENTS.md",
    "Cargo.lock",
    "Cargo.toml",
    "README.md",
    "rust-toolchain.toml",
    "scripts/ci-local.sh",
    "docs/ARCHITECTURE.md",
    "docs/E2E_TESTING.md",
    "docs/IMPLEMENTATION_PLAN.md",
    "docs/M0_EXECUTABLE_CONTRACT.md",
    "docs/RELEASE.md",
    "docs/REQUIREMENTS_TRACEABILITY.md",
    "docs/VALIDATION.md",
    "docs/DECISIONS/0002-rust-control-plane.md",
];

fn main() -> ExitCode {
    match validate() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("repository validation failed: {message}");
            ExitCode::FAILURE
        }
    }
}

fn validate() -> Result<(), String> {
    for required in REQUIRED {
        if !Path::new(required).is_file() {
            return Err(format!("required file is absent: {required}"));
        }
    }
    if Path::new(".github/workflows").exists() {
        return Err("hosted CI is prohibited while the repository is private".into());
    }
    let output = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .output()
        .map_err(|error| format!("could not enumerate repository files: {error}"))?;
    if !output.status.success() {
        return Err("git file enumeration failed".into());
    }
    for name in String::from_utf8(output.stdout)
        .map_err(|_| "git returned a non-UTF-8 path")?
        .lines()
    {
        let path = Path::new(name);
        if !path.is_file() {
            continue;
        }
        if path.extension().is_some_and(|extension| extension == "py")
            || path
                .file_name()
                .is_some_and(|file| file == "pyproject.toml")
        {
            return Err(format!(
                "non-Rust project implementation requires an approved ADR: {name}"
            ));
        }
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        if text.contains('\u{2014}') {
            return Err(format!("Unicode U+2014 is forbidden: {name}"));
        }
    }
    let plan = fs::read_to_string("docs/IMPLEMENTATION_PLAN.md")
        .map_err(|error| format!("could not read implementation plan: {error}"))?;
    if plan.contains("M0 - executable research contract (complete)")
        && plan.lines().any(|line| line.starts_with("- [ ]"))
    {
        let m0 = plan.split("## M1").next().unwrap_or(&plan);
        if m0.lines().any(|line| line.starts_with("- [ ]")) {
            return Err("M0 is marked complete with unchecked M0 requirements".into());
        }
    }
    Ok(())
}
