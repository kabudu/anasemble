use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

const REQUIRED: &[&str] = &[
    "AGENTS.md",
    "Cargo.lock",
    "Cargo.toml",
    "deny.toml",
    "README.md",
    "rust-toolchain.toml",
    "scripts/ci-local.sh",
    "scripts/ci-linux-matrix.sh",
    "docs/ARCHITECTURE.md",
    "docs/E2E_TESTING.md",
    "docs/IMPLEMENTATION_PLAN.md",
    "docs/M0_EXECUTABLE_CONTRACT.md",
    "docs/M3_DECISION.md",
    "docs/M3_DILIGENCE_LOG.md",
    "docs/P0_PRODUCTION_FOUNDATIONS.md",
    "docs/P1_EVIDENCE_PLANE.md",
    "docs/P3_ISOLATED_ACTIVATION.md",
    "docs/P4_OPERATIONS_AND_READINESS.md",
    "docs/COMPATIBILITY.md",
    "docs/INSTALLATION.md",
    "docs/LINUX_MATRIX.md",
    "docs/QUICKSTART.md",
    "docs/DISASTER_RUNBOOK.md",
    "docs/INDEPENDENT_REPRODUCTION.md",
    "docs/RELEASE.md",
    "docs/REQUIREMENTS_TRACEABILITY.md",
    "docs/VALIDATION.md",
    "docs/TCB_LEDGER.md",
    "experiments/m3-comparison.json",
    "experiments/m3-costs.json",
    "examples/service-v1.json",
    "examples/reference-recovery-config-v1.json",
    "docs/DECISIONS/0002-rust-control-plane.md",
    "assets/anasemble-mark.svg",
    "assets/anasemble-wordmark.svg",
    "release/0.1.0-rc.1.md",
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
    for completed in [
        "- [x] **P0.1 Service manifest:**",
        "- [x] **P0.2 Filesystem state adapter:**",
        "## Optional post-release assurance",
        "- [x] **P1.1 Production signatures:**",
        "- [x] **P1.2 Identity lifecycle:**",
        "- [x] **P1.3 Fragment stores:**",
        "- [x] **P1.4 Evidence protection:**",
        "- [x] **P1.5 Adversarial drill:**",
    ] {
        if !plan.contains(completed) {
            return Err(format!(
                "production roadmap invariant is absent: {completed}"
            ));
        }
    }
    if !plan.contains("Independent clean-clone reproduction attestation.")
        || !plan.contains("External security and soundness assessment.")
    {
        return Err("optional post-release assurance must remain explicit".into());
    }
    for completed in [
        "Add durable job state, restart recovery, bounded scheduling and backpressure",
        "Define compatibility, upgrade, configuration migration, backup interoperability",
        "Execute destructive staging-environment drills, sustained performance tests",
        "Complete bounded positioning, brand, adoption, packaging",
        "Exit: every production claim maps to retained executable evidence",
    ] {
        if !plan.contains(&format!("- [x] {completed}")) {
            return Err(format!("P4 completion invariant is absent: {completed}"));
        }
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
    let service: anasemble::service::ServiceManifest = serde_json::from_slice(
        &fs::read("examples/service-v1.json")
            .map_err(|error| format!("could not read service example: {error}"))?,
    )
    .map_err(|error| format!("service example is invalid JSON: {error}"))?;
    service
        .validate()
        .map_err(|error| format!("service example is invalid: {error}"))?;
    let release = fs::read_to_string("release/0.1.0-rc.1.md")
        .map_err(|error| format!("could not read release candidate notes: {error}"))?;
    for required in [
        "## Highlights",
        "## Installation",
        "## Compatibility and claims",
        "docs/COMPATIBILITY.md",
        "docs/P4_OPERATIONS_AND_READINESS.md",
    ] {
        if !release.contains(required) {
            return Err(format!(
                "release presentation section is absent: {required}"
            ));
        }
    }
    if release
        .lines()
        .next()
        .is_none_or(|line| line.starts_with('#'))
    {
        return Err("release presentation must open with the user-visible outcome".into());
    }
    Ok(())
}
