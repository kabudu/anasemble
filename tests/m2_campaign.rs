mod common;

use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use anasemble::campaign::run_campaign;
use anasemble::canonical::digest;
use anasemble::deployment::{
    FailurePoint, StateSnapshot, StateTransform, deploy, deploy_with_failure, read_active, rollback,
};
use anasemble::fragments::FragmentKind;
use anasemble::model::{FragmentContent, RefusalCode, TraceRole};
use anasemble::protocol::{RecoveryResult, run};
use common::{CONTRACT_KEY, STATE_KEY, build_workspace, grammar, write_envelope, write_json};
use serde_json::{Value, json};
use tempfile::tempdir;

#[test]
fn state_transform_deploy_partial_failure_and_rollback_are_atomic() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    let recovery = run(&workspace.recovery);
    let deployment = directory.path().join("deployment");
    let transform = transform("legacy-v1", [("closed", "locked"), ("open", "unlocked")]);
    let first = StateSnapshot {
        schema_version: "legacy-v1".into(),
        state: "open".into(),
        revision: 7,
    };
    let receipt = deploy(&deployment, &recovery, &first, &transform).unwrap();
    assert_eq!(receipt.state_revision, 8);
    assert_eq!(read_active(&deployment).unwrap().state.state, "unlocked");

    let second = StateSnapshot {
        schema_version: "legacy-v1".into(),
        state: "closed".into(),
        revision: 8,
    };
    let error = deploy_with_failure(
        &deployment,
        &recovery,
        &second,
        &transform,
        FailurePoint::BeforeActivation,
    )
    .unwrap_err();
    assert!(error.to_string().contains("injected failure"));
    assert_eq!(read_active(&deployment).unwrap().state.state, "unlocked");

    fs::write(deployment.join(".deploy.lock"), b"held").unwrap();
    assert!(deploy(&deployment, &recovery, &second, &transform).is_err());
    assert_eq!(read_active(&deployment).unwrap().state.state, "unlocked");
    fs::remove_file(deployment.join(".deploy.lock")).unwrap();

    deploy(&deployment, &recovery, &second, &transform).unwrap();
    assert_eq!(read_active(&deployment).unwrap().state.state, "locked");
    let rollback_path = deployment.join("rollback.json");
    let valid_rollback = fs::read(&rollback_path).unwrap();
    let mut corrupt: Value = serde_json::from_slice(&valid_rollback).unwrap();
    corrupt["candidate_digest"] = Value::String("00".repeat(32));
    write_json(&rollback_path, &corrupt);
    assert!(rollback(&deployment).is_err());
    fs::write(&rollback_path, valid_rollback).unwrap();
    rollback(&deployment).unwrap();
    assert_eq!(read_active(&deployment).unwrap().state.state, "unlocked");
}

#[test]
fn public_deploy_and_rollback_cli_preserve_modeled_state() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    let deployment = directory.path().join("deployment");
    let state_path = directory.path().join("state.json");
    let transform_path = directory.path().join("transform.json");
    write_json(
        &state_path,
        &StateSnapshot {
            schema_version: "legacy-v1".into(),
            state: "open".into(),
            revision: 3,
        },
    );
    write_json(
        &transform_path,
        &transform("legacy-v1", [("closed", "locked"), ("open", "unlocked")]),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .args(["deploy"])
        .arg(&workspace.recovery)
        .arg(&state_path)
        .arg(&transform_path)
        .arg(&deployment)
        .env_clear()
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(read_active(&deployment).unwrap().state.revision, 4);
}

#[test]
fn retained_m3_costs_match_fixture_payloads() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), false);
    let costs: Value = serde_json::from_slice(
        &fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("experiments/m3-costs.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let fragment_bytes: u64 = fs::read_dir(workspace.recovery.join("fragments"))
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .sum();
    assert_eq!(
        costs["artifact_bytes"],
        fs::metadata(workspace.artifact).unwrap().len()
    );
    assert_eq!(costs["semantic_fragment_bytes"], fragment_bytes);
    assert_eq!(costs["semantic_fragment_count"], 6);
}

#[test]
fn campaign_retains_positive_refusal_timeout_disagreement_and_negative_results() {
    let directory = tempdir().unwrap();
    let root = directory.path().join("campaign");
    fs::create_dir(&root).unwrap();

    let positive = build_case(&root, "positive");
    let positive_result = run(&positive);
    let RecoveryResult::Certified { candidate, .. } = &positive_result else {
        panic!("positive fixture must certify");
    };
    let expected_digest = digest(candidate.as_ref()).unwrap();

    let refusal = build_case(&root, "refusal");
    fs::remove_file(refusal.join("fragments/contract-3.json")).unwrap();

    let timeout = build_case(&root, "timeout");
    mutate_registry(&timeout, |value| {
        value["grammar"]["max_candidates"] = Value::from(1)
    });

    let disagreement = build_case(&root, "disagreement");
    mutate_registry(&disagreement, |value| {
        value["loss_oracle"]["forbidden_sha256"]
            .as_array_mut()
            .unwrap()
            .push(Value::String(expected_digest.clone()));
    });

    let negative = build_case(&root, "negative");
    let fragment = negative.join("fragments/contract-0.json");
    let mut poisoned: Value = serde_json::from_slice(&fs::read(&fragment).unwrap()).unwrap();
    poisoned["content"]["output"] = Value::String("locked".into());
    write_json(&fragment, &poisoned);

    write_json(
        &root.join("campaign.json"),
        &json!({
            "version": "campaign-v1",
            "cases": [
                {"id": "positive", "workspace": "positive", "expected": "certified", "expected_candidate_digest": expected_digest},
                {"id": "refusal", "workspace": "refusal", "expected": "refused"},
                {"id": "timeout", "workspace": "timeout", "expected": "timeout"},
                {"id": "disagreement", "workspace": "disagreement", "expected": "disagreement"},
                {"id": "negative", "workspace": "negative", "expected": "negative"}
            ]
        }),
    );
    let report = run_campaign(&root).unwrap();
    assert!(report.cases.iter().all(|case| case.matched_expectation));
    assert_eq!(report.metrics.total_cases, 5);
    assert_eq!(report.metrics.certified_correct_recoveries, 1);
    assert_eq!(report.metrics.unsafe_certifications, 0);
    assert_eq!(report.metrics.timeouts, 1);
    assert_eq!(report.metrics.disagreements, 1);
    assert_eq!(report.metrics.retained_negative_results, 1);
    assert!(report.cases.iter().all(|case| case.baselines.len() == 3));
    assert_eq!(
        report.registered_primary_metrics,
        ["certified-correct-recoveries", "unsafe-certifications"]
    );
    let comparison: Value = serde_json::from_slice(
        &fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("experiments/m3-comparison.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let baseline_count = |name: &str, observed: &str| {
        report
            .cases
            .iter()
            .flat_map(|case| &case.baselines)
            .filter(|baseline| baseline.name == name && baseline.observed == observed)
            .count() as u64
    };
    let baseline_refusals = |name: &str| {
        report
            .cases
            .iter()
            .flat_map(|case| &case.baselines)
            .filter(|baseline| baseline.name == name && baseline.observed.starts_with("refused:"))
            .count() as u64
    };
    assert_eq!(comparison["methods"]["anasemble"]["certified"], 1);
    assert_eq!(
        comparison["methods"]["backup_replica"]["unavailable"],
        baseline_count("backup-replica", "unavailable_after_registered_total_loss")
    );
    assert_eq!(
        comparison["methods"]["trace_only"]["certified"],
        baseline_count("trace-only", "certified")
    );
    assert_eq!(
        comparison["methods"]["centralized_contract"]["certified"],
        baseline_count("centralized-contract", "certified")
    );
    assert_eq!(
        comparison["methods"]["trace_only"]["refused"],
        baseline_refusals("trace-only")
    );
    assert_eq!(
        comparison["methods"]["centralized_contract"]["refused"],
        baseline_refusals("centralized-contract")
    );
    let output = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .arg("evaluate-campaign")
        .arg(&root)
        .env_clear()
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let public_report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(public_report["metrics"]["unsafe_certifications"], 0);
    let manifest_path = root.join("campaign.json");
    let mut permissive: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let mut over_budget = permissive.clone();
    permissive["cases"][0]
        .as_object_mut()
        .unwrap()
        .remove("expected_candidate_digest");
    write_json(&manifest_path, &permissive);
    assert!(run_campaign(&root).is_err());
    mutate_registry(&positive, |value| {
        value["grammar"]["max_candidates"] = Value::from(1_000_000);
    });
    mutate_registry(&refusal, |value| {
        value["grammar"]["max_candidates"] = Value::from(1_000_000);
    });
    over_budget["cases"].as_array_mut().unwrap().truncate(2);
    write_json(&manifest_path, &over_budget);
    assert!(run_campaign(&root).is_err());
}

#[test]
fn evidence_campaign_refuses_poison_omission_contradiction_replay_and_staleness() {
    let directory = tempdir().unwrap();

    let poisoned = build_workspace(&directory.path().join("poisoned"), true).recovery;
    let path = poisoned.join("fragments/contract-0.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["content"]["output"] = Value::String("locked".into());
    write_json(&path, &value);
    assert_refusal(run(&poisoned), RefusalCode::InvalidEvidence);

    let omitted = build_workspace(&directory.path().join("omitted"), true).recovery;
    fs::remove_file(omitted.join("fragments/contract-3.json")).unwrap();
    assert_refusal(run(&omitted), RefusalCode::InsufficientEvidence);

    let contradictory = build_workspace(&directory.path().join("contradictory"), true).recovery;
    write_envelope(
        &contradictory.join("fragments/contract-3.json"),
        FragmentKind::Contract,
        "contract-authority",
        "domain-a",
        3,
        FragmentContent::Transition {
            state: "locked".into(),
            input: "coin".into(),
            next_state: "locked".into(),
            output: "locked".into(),
        },
        &CONTRACT_KEY,
    );
    assert_refusal(run(&contradictory), RefusalCode::ContradictoryEvidence);

    let replayed = build_workspace(&directory.path().join("replayed"), true).recovery;
    fs::copy(
        replayed.join("fragments/contract-0.json"),
        replayed.join("fragments/replay.json"),
    )
    .unwrap();
    assert_refusal(run(&replayed), RefusalCode::InvalidEvidence);

    let stale = build_workspace(&directory.path().join("stale"), true).recovery;
    mutate_registry(&stale, |value| {
        value["evidence_window"] = json!({
            "not_before": "2026-08-01T00:00:00+00:00",
            "not_after": "2026-08-31T00:00:00+00:00"
        });
    });
    assert_refusal(run(&stale), RefusalCode::InvalidEvidence);
}

#[test]
fn trust_campaign_refuses_shared_domain_forgery_and_trace_overfitting() {
    let directory = tempdir().unwrap();
    let shared = build_workspace(&directory.path().join("shared"), true).recovery;
    mutate_registry(&shared, |value| {
        value["trusted_issuers"]["state-authority"]["failure_domain"] =
            Value::String("domain-a".into());
    });
    write_envelope(
        &shared.join("fragments/state.json"),
        FragmentKind::StateSchema,
        "state-authority",
        "domain-a",
        0,
        FragmentContent::StatePolicy {
            states: grammar().states,
            initial_state: "locked".into(),
        },
        &STATE_KEY,
    );
    write_envelope(
        &shared.join("fragments/held-out-trace.json"),
        FragmentKind::Trace,
        "state-authority",
        "domain-a",
        1,
        FragmentContent::Trace {
            role: TraceRole::HeldOut,
            initial_state: "locked".into(),
            inputs: vec!["coin".into()],
            outputs: vec!["unlocked".into()],
        },
        &STATE_KEY,
    );
    assert_refusal(run(&shared), RefusalCode::InsufficientEvidence);

    let forged = build_workspace(&directory.path().join("forged"), true).recovery;
    let path = forged.join("fragments/contract-0.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["issuer"] = Value::String("state-authority".into());
    write_json(&path, &value);
    assert_refusal(run(&forged), RefusalCode::InvalidEvidence);

    let overfit = build_workspace(&directory.path().join("overfit"), true).recovery;
    write_envelope(
        &overfit.join("fragments/held-out-trace.json"),
        FragmentKind::Trace,
        "state-authority",
        "domain-b",
        1,
        FragmentContent::Trace {
            role: TraceRole::HeldOut,
            initial_state: "locked".into(),
            inputs: vec!["coin".into()],
            outputs: vec!["locked".into()],
        },
        &STATE_KEY,
    );
    assert_refusal(run(&overfit), RefusalCode::CheckerRejected);
}

fn build_case(root: &std::path::Path, name: &str) -> std::path::PathBuf {
    let source = root.join(format!(".source-{name}"));
    let fixture = build_workspace(&source, true);
    assert!(!fixture.artifact.exists());
    assert_eq!(fixture.artifact_digest.len(), 64);
    let workspace = fixture.recovery;
    let target = root.join(name);
    fs::rename(workspace, &target).unwrap();
    fs::remove_dir(source).unwrap();
    target
}

fn mutate_registry(workspace: &std::path::Path, edit: impl FnOnce(&mut Value)) {
    let path = workspace.join("registry.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    edit(&mut value);
    write_json(&path, &value);
}

fn transform<const N: usize>(from: &str, states: [(&str, &str); N]) -> StateTransform {
    StateTransform {
        from_schema: from.into(),
        to_schema: grammar().version,
        state_map: states
            .into_iter()
            .map(|(source, target)| (source.into(), target.into()))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn assert_refusal(result: RecoveryResult, expected: RefusalCode) {
    let RecoveryResult::Refused { code, .. } = result else {
        panic!("expected refusal");
    };
    assert_eq!(code, expected);
}
