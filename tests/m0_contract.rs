mod common;

use std::fs;

use anasemble::canonical::encode;
use anasemble::checker::certify;
use anasemble::fragments::{Envelope, FragmentKind, IssuerPolicy, collect, sign};
use anasemble::model::{Candidate, Error, RefusalCode};
use anasemble::oracle::attest_absence;
use anasemble::protocol::{RecoveryResult, run};
use common::{CONTRACT_KEY, build_workspace, grammar, transitions, write_envelope, write_json};
use serde_json::Value;
use std::collections::BTreeMap;
use tempfile::tempdir;

#[test]
fn reconstructs_and_certifies_turnstile_after_loss() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    let result = run(&workspace.recovery);
    let RecoveryResult::Certified {
        candidate,
        certificate,
    } = result
    else {
        panic!("expected certification");
    };
    assert_eq!(candidate.transitions.len(), 4);
    assert!(!workspace.artifact.exists());
    assert!(
        !serde_json::to_string(&candidate)
            .unwrap()
            .contains(&workspace.artifact_digest)
    );
    let value = serde_json::to_value(certificate).unwrap();
    assert_eq!(value["coverage"]["passed_obligations"], 4);
    assert_eq!(value["coverage"]["passed_held_out_traces"], 1);
    assert_eq!(value["non_identical_to_forbidden_artifacts"], true);
    assert!(value["search_bounds"]["examined"].as_u64().unwrap() <= 16);
}

#[test]
fn canonical_json_sorts_object_keys() {
    let value = serde_json::json!({"z": 1, "a": {"y": 2, "b": 3}});
    assert_eq!(encode(&value).unwrap(), br#"{"a":{"b":3,"y":2},"z":1}"#);
}

#[test]
fn refuses_when_original_artifact_exists() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), false);
    assert_refusal(run(&workspace.recovery), RefusalCode::ArtifactPresent);
}

#[test]
fn refuses_missing_contract() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    fs::remove_file(workspace.recovery.join("fragments/contract-3.json")).unwrap();
    assert_refusal(run(&workspace.recovery), RefusalCode::InsufficientEvidence);
}

#[test]
fn refuses_tampered_fragment() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    let path = workspace.recovery.join("fragments/contract-0.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["content"]["output"] = Value::String("locked".into());
    write_json(&path, &value);
    assert_refusal(run(&workspace.recovery), RefusalCode::InvalidEvidence);
}

#[test]
fn refuses_kind_content_mismatch() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    write_envelope(
        &workspace.recovery.join("fragments/contract-0.json"),
        FragmentKind::Trace,
        "contract-authority",
        "domain-a",
        0,
        transitions().remove(0),
        &CONTRACT_KEY,
    );
    assert_refusal(run(&workspace.recovery), RefusalCode::InvalidEvidence);
}

#[test]
fn malformed_fragment_is_evidence_failure_not_registry_failure() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    fs::write(
        workspace.recovery.join("fragments/contract-0.json"),
        b"{not-json",
    )
    .unwrap();
    assert_refusal(run(&workspace.recovery), RefusalCode::InvalidEvidence);
}

#[test]
fn refuses_forged_domain_even_with_valid_signature() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    write_envelope(
        &workspace.recovery.join("fragments/contract-0.json"),
        FragmentKind::Contract,
        "contract-authority",
        "forged-domain",
        0,
        transitions().remove(0),
        &CONTRACT_KEY,
    );
    assert_refusal(run(&workspace.recovery), RefusalCode::InvalidEvidence);
}

#[test]
fn refuses_search_budget_exhaustion() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    let path = workspace.recovery.join("registry.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["grammar"]["max_candidates"] = Value::from(1);
    write_json(&path, &value);
    assert_refusal(run(&workspace.recovery), RefusalCode::SearchExhausted);
}

#[test]
fn refuses_contradictory_transition_obligations() {
    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    write_envelope(
        &workspace.recovery.join("fragments/contract-3.json"),
        FragmentKind::Contract,
        "contract-authority",
        "domain-a",
        3,
        anasemble::model::FragmentContent::Transition {
            state: "locked".into(),
            input: "coin".into(),
            next_state: "locked".into(),
            output: "locked".into(),
        },
        &CONTRACT_KEY,
    );
    assert_refusal(run(&workspace.recovery), RefusalCode::ContradictoryEvidence);
}

#[test]
fn collector_refuses_dependency_cycle() {
    let base = Envelope {
        kind: FragmentKind::Contract,
        component: "turnstile".into(),
        interface_version: "1".into(),
        issuer: "contract-authority".into(),
        failure_domain: "domain-a".into(),
        issued_at: "2026-07-29T00:00:00+00:00".into(),
        sequence: 0,
        content_digest: String::new(),
        dependencies: Vec::new(),
        content: transitions().remove(0),
        signature: String::new(),
    };
    let other = Envelope {
        sequence: 1,
        content: transitions().remove(1),
        ..base.clone()
    };
    let first_digest = sign(base.clone(), &CONTRACT_KEY).unwrap().content_digest;
    let second_digest = sign(other.clone(), &CONTRACT_KEY).unwrap().content_digest;
    let first = sign(
        Envelope {
            dependencies: vec![second_digest],
            ..base
        },
        &CONTRACT_KEY,
    )
    .unwrap();
    let second = sign(
        Envelope {
            dependencies: vec![first_digest],
            ..other
        },
        &CONTRACT_KEY,
    )
    .unwrap();
    let policies = BTreeMap::from([(
        "contract-authority".into(),
        IssuerPolicy {
            hmac_sha256_key: hex::encode(CONTRACT_KEY),
            failure_domain: "domain-a".into(),
        },
    )]);
    let error = collect(vec![first, second], &policies, 1, "turnstile", "1").unwrap_err();
    assert!(matches!(error, Error::InvalidEvidence(_)));
    assert!(error.to_string().contains("cycle"));
}

#[test]
fn checker_rejects_mutated_candidate() {
    let mut candidate = Candidate {
        component: "turnstile".into(),
        interface_version: "1".into(),
        grammar: grammar(),
        transitions: transitions()
            .iter()
            .filter_map(anasemble::model::FragmentContent::transition)
            .collect(),
    };
    candidate.transitions[0].output = "locked".into();
    let mut contents = transitions();
    contents.push(anasemble::model::FragmentContent::Trace {
        initial_state: "locked".into(),
        inputs: vec!["coin".into()],
        outputs: vec!["unlocked".into()],
    });
    contents.push(anasemble::model::FragmentContent::StatePolicy {
        states: grammar().states,
        initial_state: "locked".into(),
    });
    let error = certify(
        &serde_json::to_vec(&candidate).unwrap(),
        "turnstile",
        "1",
        &contents,
    )
    .unwrap_err();
    assert!(matches!(error, Error::CheckerRejected(_)));
}

#[test]
fn checker_binds_candidate_identity() {
    let candidate = Candidate {
        component: "different-component".into(),
        interface_version: "1".into(),
        grammar: grammar(),
        transitions: transitions()
            .iter()
            .filter_map(anasemble::model::FragmentContent::transition)
            .collect(),
    };
    let error = certify(
        &serde_json::to_vec(&candidate).unwrap(),
        "turnstile",
        "1",
        &[],
    )
    .unwrap_err();
    assert!(matches!(error, Error::CheckerRejected(_)));
    assert!(error.to_string().contains("identity"));
}

#[cfg(unix)]
#[test]
fn oracle_rejects_symlink() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().unwrap();
    let recovery = directory.path().join("recovery");
    fs::create_dir(&recovery).unwrap();
    let outside = directory.path().join("outside");
    fs::write(&outside, b"outside").unwrap();
    symlink(&outside, recovery.join("link")).unwrap();
    let error = attest_absence(&recovery, &[], &[], 10, 1024).unwrap_err();
    assert!(matches!(error, Error::InvalidEvidence(_)));
}

#[test]
fn oracle_refuses_during_bounded_directory_traversal() {
    let directory = tempdir().unwrap();
    let recovery = directory.path().join("recovery");
    fs::create_dir_all(recovery.join("one/two/three")).unwrap();
    let error = attest_absence(&recovery, &[], &[], 2, 1024).unwrap_err();
    assert!(matches!(error, Error::SearchExhausted(_)));
}

fn assert_refusal(result: RecoveryResult, expected: RefusalCode) {
    let RecoveryResult::Refused { code, .. } = result else {
        panic!("expected refusal");
    };
    assert_eq!(code, expected);
}
