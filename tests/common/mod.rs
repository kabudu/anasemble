use std::fs;
use std::path::{Path, PathBuf};

use anasemble::canonical::{bytes_digest, encode};
use anasemble::fragments::{Envelope, FragmentKind, sign};
use anasemble::model::{FragmentContent, Grammar};
use serde::Serialize;
use serde_json::json;

pub const CONTRACT_KEY: [u8; 32] = [0x11; 32];
pub const STATE_KEY: [u8; 32] = [0x22; 32];

pub fn grammar() -> Grammar {
    Grammar {
        version: "fsm-v0".into(),
        inputs: vec!["coin".into(), "push".into()],
        outputs: vec!["locked".into(), "unlocked".into()],
        states: vec!["locked".into(), "unlocked".into()],
        initial_state: "locked".into(),
        max_candidates: 16,
    }
}

pub fn transitions() -> Vec<FragmentContent> {
    vec![
        transition("locked", "coin", "unlocked", "unlocked"),
        transition("locked", "push", "locked", "locked"),
        transition("unlocked", "coin", "unlocked", "unlocked"),
        transition("unlocked", "push", "locked", "locked"),
    ]
}

fn transition(state: &str, input: &str, next_state: &str, output: &str) -> FragmentContent {
    FragmentContent::Transition {
        state: state.into(),
        input: input.into(),
        next_state: next_state.into(),
        output: output.into(),
    }
}

pub struct Workspace {
    pub recovery: PathBuf,
    pub artifact: PathBuf,
    pub artifact_digest: String,
}

pub fn build_workspace(base: &Path, delete_artifact: bool) -> Workspace {
    let origin = base.join("origin");
    let recovery = base.join("recovery");
    let fragments = recovery.join("fragments");
    fs::create_dir_all(&origin).unwrap();
    fs::create_dir_all(&fragments).unwrap();
    let artifact = origin.join("turnstile.component.json");
    write_json(
        &artifact,
        &json!({"component": "turnstile", "grammar": grammar(), "transitions": transitions()}),
    );
    let artifact_digest = bytes_digest(&fs::read(&artifact).unwrap());
    for (index, content) in transitions().into_iter().enumerate() {
        write_envelope(
            &fragments.join(format!("contract-{index}.json")),
            FragmentKind::Contract,
            "contract-authority",
            "domain-a",
            index as u64,
            content,
            &CONTRACT_KEY,
        );
    }
    write_envelope(
        &fragments.join("state.json"),
        FragmentKind::StateSchema,
        "state-authority",
        "domain-b",
        0,
        FragmentContent::StatePolicy {
            states: grammar().states,
            initial_state: "locked".into(),
        },
        &STATE_KEY,
    );
    write_envelope(
        &fragments.join("held-out-trace.json"),
        FragmentKind::Trace,
        "state-authority",
        "domain-b",
        1,
        FragmentContent::Trace {
            initial_state: "locked".into(),
            inputs: vec!["coin".into(), "push".into(), "push".into()],
            outputs: vec!["unlocked".into(), "locked".into(), "locked".into()],
        },
        &STATE_KEY,
    );
    write_json(
        &recovery.join("registry.json"),
        &json!({
            "component": "turnstile",
            "interface_version": "1",
            "grammar": grammar(),
            "required_domains": 2,
            "trusted_issuers": {
                "contract-authority": {"hmac_sha256_key": hex::encode(CONTRACT_KEY), "failure_domain": "domain-a"},
                "state-authority": {"hmac_sha256_key": hex::encode(STATE_KEY), "failure_domain": "domain-b"}
            },
            "loss_oracle": {
                "forbidden_paths": [artifact, origin],
                "forbidden_sha256": [artifact_digest]
            },
            "resource_limits": {
                "max_fragments": 16,
                "max_fragment_bytes": 16384,
                "max_workspace_files": 32,
                "max_workspace_bytes": 262144
            },
            "experiment": {
                "seed": 20260729,
                "baselines": ["backup-replica", "trace-only", "centralized-contract"],
                "primary_metrics": ["certified-correct-recoveries", "unsafe-certifications"],
                "secondary_metrics": ["refusal-rate", "search-time", "candidate-complexity", "authoring-cost"]
            }
        }),
    );
    if delete_artifact {
        fs::remove_dir_all(&origin).unwrap();
    }
    Workspace {
        recovery,
        artifact,
        artifact_digest,
    }
}

pub fn write_envelope(
    path: &Path,
    kind: FragmentKind,
    issuer: &str,
    domain: &str,
    sequence: u64,
    content: FragmentContent,
    key: &[u8; 32],
) {
    let envelope = sign(
        Envelope {
            kind,
            component: "turnstile".into(),
            interface_version: "1".into(),
            issuer: issuer.into(),
            failure_domain: domain.into(),
            issued_at: "2026-07-29T00:00:00+00:00".into(),
            sequence,
            content_digest: String::new(),
            dependencies: Vec::new(),
            content,
            signature: String::new(),
        },
        key,
    )
    .unwrap();
    write_json(path, &envelope);
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) {
    let mut data = encode(value).unwrap();
    data.push(b'\n');
    fs::write(path, data).unwrap();
}
