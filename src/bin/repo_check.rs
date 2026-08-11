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
    "docs/M3_DECISION.md",
    "docs/M3_DILIGENCE_LOG.md",
    "docs/INDEPENDENT_REPRODUCTION.md",
    "docs/RELEASE.md",
    "docs/REQUIREMENTS_TRACEABILITY.md",
    "docs/VALIDATION.md",
    "docs/TCB_LEDGER.md",
    "experiments/m3-comparison.json",
    "experiments/m3-costs.json",
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
    for section in plan.split("\n## ") {
        if section
            .lines()
            .next()
            .is_some_and(|line| line.contains("(complete)"))
            && section.lines().any(|line| line.starts_with("- [ ]"))
        {
            return Err("a completed milestone contains unchecked requirements".into());
        }
    }
    if !plan.contains(
        "- [ ] Obtain reproduction and security/soundness review by an independent party.",
    ) || !plan.contains("continue as research")
    {
        return Err("M3 must retain its independent-review gate and research-only decision".into());
    }
    let comparison: serde_json::Value = serde_json::from_slice(
        &fs::read("experiments/m3-comparison.json")
            .map_err(|error| format!("could not read M3 comparison: {error}"))?,
    )
    .map_err(|error| format!("M3 comparison is invalid JSON: {error}"))?;
    if comparison["methods"]["anasemble"]["certified"] != 1
        || comparison["methods"]["centralized_contract"]["certified"] != 1
    {
        return Err("M3 comparison must retain the matched centralized result".into());
    }
    Ok(())
}
