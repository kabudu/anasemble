use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

const REQUIRED: &[&str] = &[
    "AGENTS.md",
    "Cargo.lock",
    "Cargo.toml",
    "LICENSE",
    "NOTICE",
    "TRADEMARKS.md",
    "CONTRIBUTING.md",
    "CODE_OF_CONDUCT.md",
    "SECURITY.md",
    "SUPPORT.md",
    "GOVERNANCE.md",
    ".github/PULL_REQUEST_TEMPLATE.md",
    ".github/ISSUE_TEMPLATE/bug_report.yml",
    ".github/ISSUE_TEMPLATE/config.yml",
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
    "docs/FINAL_SECURITY_SWEEP.md",
    "docs/LINUX_MATRIX.md",
    "docs/PRODUCTISATION.md",
    "docs/PUBLIC_OPENING.md",
    "docs/BRAND_IDENTITY.md",
    "docs/BRAND_VALIDATION.md",
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
    "assets/brand/BRAND_ASSET_MANIFEST.json",
    "assets/brand/source/anasemble-symbol.svg",
    "assets/brand/source/anasemble-horizontal.svg",
    "assets/brand/source/anasemble-result-icons.svg",
    "assets/brand/source/anasemble-stacked.svg",
    "assets/brand/source/anasemble-symbol-mono.svg",
    "assets/brand/source/anasemble-symbol-reversed.svg",
    "assets/brand/source/anasemble-small.svg",
    "assets/brand/tokens/brand-tokens.json",
    "assets/brand/tokens/brand-tokens.css",
    "assets/brand/templates/diagram-key.svg",
    "assets/brand/templates/chart-key.svg",
    "assets/brand/templates/release-card.svg",
    "assets/brand/templates/social-card.svg",
    "assets/brand/templates/presentation-title.svg",
    "assets/brand/LICENSES/OWNED-ASSETS.md",
    "release/0.1.0-rc.1.md",
    "release/0.1.0-rc.1.title",
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
    anasemble::brand::validate(Path::new("."))?;
    validate_open_source_metadata()?;
    validate_product_readme()?;
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
        "Complete bounded positioning, full brand identity, adoption, packaging",
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

fn validate_product_readme() -> Result<(), String> {
    let readme = fs::read_to_string("README.md")
        .map_err(|error| format!("could not read README: {error}"))?;
    let opening = readme.lines().take(24).collect::<Vec<_>>().join("\n");
    for required in [
        "assets/anasemble-mark.svg",
        "Recover a lost service component",
        "## What Anasemble does",
        "## Supported today",
        "## Quick start",
        "## Safety and scope",
        "docs/COMPATIBILITY.md",
        "docs/QUICKSTART.md",
    ] {
        if !readme.contains(required) {
            return Err(format!("product README section is absent: {required}"));
        }
    }
    if opening.contains("research project")
        || opening.contains("Candidate contribution")
        || opening.contains("M0 through")
    {
        return Err("README opening must lead with the product outcome".into());
    }
    Ok(())
}

fn validate_open_source_metadata() -> Result<(), String> {
    let manifest = fs::read_to_string("Cargo.toml")
        .map_err(|error| format!("could not read Cargo.toml: {error}"))?;
    for required in [
        "version = \"0.1.0-rc.1\"",
        "license = \"Apache-2.0\"",
        "repository = \"https://github.com/kabudu/anasemble\"",
        "publish = false",
    ] {
        if !manifest.contains(required) {
            return Err(format!(
                "open-source package metadata is absent: {required}"
            ));
        }
    }
    let license = fs::read_to_string("LICENSE")
        .map_err(|error| format!("could not read LICENSE: {error}"))?;
    if !license.contains("Apache License")
        || !license.contains("Version 2.0, January 2004")
        || !license.contains("END OF TERMS AND CONDITIONS")
    {
        return Err("LICENSE is not the complete Apache License 2.0 text".into());
    }
    let title = fs::read_to_string("release/0.1.0-rc.1.title")
        .map_err(|error| format!("could not read release title: {error}"))?;
    if title.trim() != "Anasemble v0.1.0-rc.1: Evidence-bound recovery"
        || title.lines().count() != 1
    {
        return Err("curated release title is missing or malformed".into());
    }
    let notes = fs::read_to_string("release/0.1.0-rc.1.md")
        .map_err(|error| format!("could not read release notes: {error}"))?;
    if notes.lines().any(|line| line.starts_with("# "))
        || notes.contains(title.trim())
        || notes
            .lines()
            .any(|line| line.starts_with("- ") && line.len() > 500)
    {
        return Err("curated release notes violate presentation policy".into());
    }
    for required in ["LICENSE", "NOTICE", "SECURITY.md", "CONTRIBUTING.md"] {
        let output = Command::new("git")
            .args(["check-ignore", "-q", required])
            .status()
            .map_err(|error| format!("could not check source distribution: {error}"))?;
        if output.success() {
            return Err(format!("release file is excluded from source: {required}"));
        }
    }
    Ok(())
}
