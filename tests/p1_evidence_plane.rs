#[allow(dead_code)]
mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anasemble::evidence_plane::{
    RecoveryKeyFile, SealedEvidence, StoreBundle, retrieve, seal, sign_bundle,
};
use anasemble::fragments::{
    Ed25519IssuerPolicy, Ed25519KeyPolicy, Envelope, FragmentKind, IssuerPolicy, collect,
    collect_at, sign_ed25519,
};
use anasemble::model::{FragmentContent, TraceRole};
use ed25519_dalek::SigningKey;
use serde_json::{Value, json};
use tempfile::tempdir;

use common::{grammar, transitions, write_json};

const CONTRACT_SECRET: [u8; 32] = [0x31; 32];
const STATE_SECRET: [u8; 32] = [0x32; 32];
const STORE_A_SECRET: [u8; 32] = [0x41; 32];
const STORE_B_SECRET: [u8; 32] = [0x42; 32];
const STORE_C_SECRET: [u8; 32] = [0x43; 32];

fn policy(secret: &[u8; 32], key_id: &str, domain: &str) -> IssuerPolicy {
    IssuerPolicy::Ed25519(Ed25519IssuerPolicy {
        failure_domain: domain.into(),
        minimum_sequence: 0,
        keys: vec![Ed25519KeyPolicy {
            key_id: key_id.into(),
            public_key: hex::encode(SigningKey::from_bytes(secret).verifying_key().to_bytes()),
            not_before: "2026-01-01T00:00:00+00:00".into(),
            not_after: "2027-01-01T00:00:00+00:00".into(),
            revoked_at: None,
        }],
    })
}

fn envelope(
    kind: FragmentKind,
    issuer: &str,
    domain: &str,
    sequence: u64,
    content: FragmentContent,
    key_id: &str,
    secret: &[u8; 32],
) -> Envelope {
    sign_ed25519(
        Envelope {
            kind,
            component: "turnstile".into(),
            interface_version: "1".into(),
            issuer: issuer.into(),
            failure_domain: domain.into(),
            issued_at: "2026-08-11T00:00:00+00:00".into(),
            sequence,
            content_digest: String::new(),
            dependencies: Vec::new(),
            content,
            signature: String::new(),
        },
        key_id,
        secret,
    )
    .unwrap()
}

fn evidence() -> Vec<Envelope> {
    let mut values = transitions()
        .into_iter()
        .enumerate()
        .map(|(index, content)| {
            envelope(
                FragmentKind::Contract,
                "contract-authority",
                "issuer-domain-a",
                index as u64,
                content,
                "contract-2026",
                &CONTRACT_SECRET,
            )
        })
        .collect::<Vec<_>>();
    values.push(envelope(
        FragmentKind::StateSchema,
        "state-authority",
        "issuer-domain-b",
        0,
        FragmentContent::StatePolicy {
            states: grammar().states,
            initial_state: "locked".into(),
        },
        "state-2026",
        &STATE_SECRET,
    ));
    values.push(envelope(
        FragmentKind::Trace,
        "state-authority",
        "issuer-domain-b",
        1,
        FragmentContent::Trace {
            role: TraceRole::HeldOut,
            initial_state: "locked".into(),
            inputs: vec!["coin".into()],
            outputs: vec!["unlocked".into()],
        },
        "state-2026",
        &STATE_SECRET,
    ));
    values
}

fn recovery_key(root: &Path) -> (PathBuf, RecoveryKeyFile) {
    let path = root.join("recovery-key.json");
    let key = RecoveryKeyFile {
        version: "evidence-key-v1".into(),
        key_id: "recovery-2026".into(),
        key_hex: "55".repeat(32),
        created_at: "2026-08-01T00:00:00+00:00".into(),
    };
    write_secret(&path, &serde_json::to_vec(&key).unwrap());
    (path, key)
}

fn write_secret(path: &Path, data: &[u8]) {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        file.write_all(data).unwrap();
    }
    #[cfg(not(unix))]
    fs::write(path, data).unwrap();
}

fn sealed(key: &RecoveryKeyFile) -> Vec<SealedEvidence> {
    evidence()
        .iter()
        .map(|envelope| {
            seal(
                envelope,
                key,
                "2026-08-11T00:00:00+00:00",
                "2026-09-01T00:00:00+00:00",
            )
            .unwrap()
        })
        .collect()
}

fn store(
    root: &Path,
    id: &str,
    domain: &str,
    secret: &[u8; 32],
    evidence: Vec<SealedEvidence>,
) -> PathBuf {
    let directory = root.join(id);
    fs::create_dir(&directory).unwrap();
    let bundle = sign_bundle(
        StoreBundle {
            version: "fragment-store-v1".into(),
            store_id: id.into(),
            failure_domain: domain.into(),
            generation: 7,
            evidence,
            signature: String::new(),
        },
        secret,
    )
    .unwrap();
    write_json(&directory.join("bundle.json"), &bundle);
    directory
}

fn config(root: &Path, key_path: &Path, stores: Value, policies: Value) -> PathBuf {
    let path = root.join("evidence-config.json");
    write_json(
        &path,
        &json!({
            "component": "turnstile",
            "interface_version": "1",
            "required_fragment_domains": 2,
            "required_stores": 2,
            "required_copies": 2,
            "max_parallel": 2,
            "timeout_ms": 1000,
            "retry_budget": 1,
            "verification_time": "2026-08-12T00:00:00+00:00",
            "stores": stores,
            "trusted_issuers": policies,
            "recovery_keys": {"recovery-2026": key_path}
        }),
    );
    path
}

fn policies() -> Value {
    serde_json::to_value(BTreeMap::from([
        (
            "contract-authority".to_string(),
            policy(&CONTRACT_SECRET, "contract-2026", "issuer-domain-a"),
        ),
        (
            "state-authority".to_string(),
            policy(&STATE_SECRET, "state-2026", "issuer-domain-b"),
        ),
    ]))
    .unwrap()
}

fn stores(a: &Path, b: &Path, missing: &Path) -> Value {
    json!([
        {
            "store_id": "store-a",
            "failure_domain": "admin-a",
            "public_key": hex::encode(SigningKey::from_bytes(&STORE_A_SECRET).verifying_key().to_bytes()),
            "minimum_generation": 7,
            "transport": {"type": "local_directory", "path": a}
        },
        {
            "store_id": "store-b",
            "failure_domain": "admin-b",
            "public_key": hex::encode(SigningKey::from_bytes(&STORE_B_SECRET).verifying_key().to_bytes()),
            "minimum_generation": 7,
            "transport": {"type": "local_directory", "path": b}
        },
        {
            "store_id": "store-lost",
            "failure_domain": "admin-lost",
            "public_key": "66".repeat(32),
            "minimum_generation": 1,
            "transport": {"type": "local_directory", "path": missing}
        }
    ])
}

#[test]
fn multi_domain_drill_survives_one_store_loss_and_materializes_then_deletes() {
    let directory = tempdir().unwrap();
    let (key_path, key) = recovery_key(directory.path());
    let a = store(
        directory.path(),
        "store-a",
        "admin-a",
        &STORE_A_SECRET,
        sealed(&key),
    );
    let b = store(
        directory.path(),
        "store-b",
        "admin-b",
        &STORE_B_SECRET,
        sealed(&key),
    );
    let missing = directory.path().join("lost");
    let config = config(
        directory.path(),
        &key_path,
        stores(&a, &b, &missing),
        policies(),
    );
    let retrieved = retrieve(&config).unwrap();
    assert_eq!(retrieved.receipt.successful_stores, 2);
    assert_eq!(retrieved.receipt.failed_stores, ["store-lost"]);
    assert_eq!(retrieved.receipt.envelope_count, 6);
    assert_eq!(retrieved.receipt.verification_audit.len(), 6);

    let output = directory.path().join("materialized");
    let materialize = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .args(["retrieve-evidence"])
        .arg(&config)
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        materialize.status.success(),
        "{}",
        String::from_utf8_lossy(&materialize.stderr)
    );
    let receipt: Value = serde_json::from_slice(&materialize.stdout).unwrap();
    assert_eq!(receipt["envelope_count"], 6);
    assert!(output.join("fragments/fragment-00000.json").is_file());
    assert!(output.join("receipt.json").is_file());
    let deletion = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .args(["delete-evidence"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        deletion.status.success(),
        "{}",
        String::from_utf8_lossy(&deletion.stderr)
    );
    let deletion_receipt: Value = serde_json::from_slice(&deletion.stdout).unwrap();
    assert_eq!(deletion_receipt["removed_files"], 7);
    assert!(!output.exists());
}

#[test]
fn public_key_creation_uses_restrictive_files_and_never_prints_secrets() {
    let directory = tempdir().unwrap();
    for (command, name, key_id) in [
        ("create-signing-key", "issuer-key.json", "issuer-2026"),
        ("create-recovery-key", "recovery-key.json", "recovery-2026"),
    ] {
        let path = directory.path().join(name);
        let output = Command::new(env!("CARGO_BIN_EXE_anasemble"))
            .arg(command)
            .arg(&path)
            .args([key_id, "2026-08-11T00:00:00+00:00"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(key_id));
        assert!(!stdout.contains("secret"));
        assert!(!stdout.contains("key_hex"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o077, 0);
        }
    }
}

#[test]
fn compromised_store_signature_prevents_quorum() {
    let directory = tempdir().unwrap();
    let (key_path, key) = recovery_key(directory.path());
    let a = store(
        directory.path(),
        "store-a",
        "admin-a",
        &STORE_A_SECRET,
        sealed(&key),
    );
    let b = store(
        directory.path(),
        "store-b",
        "admin-b",
        &STORE_B_SECRET,
        sealed(&key),
    );
    let path = b.join("bundle.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["generation"] = Value::from(8);
    write_json(&path, &value);
    let missing = directory.path().join("lost");
    let config = config(
        directory.path(),
        &key_path,
        stores(&a, &b, &missing),
        policies(),
    );
    let error = match retrieve(&config) {
        Ok(_) => panic!("compromised store unexpectedly reached quorum"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("quorum"));
}

#[test]
fn every_fragment_requires_copy_quorum() {
    let directory = tempdir().unwrap();
    let (key_path, key) = recovery_key(directory.path());
    let a = store(
        directory.path(),
        "store-a",
        "admin-a",
        &STORE_A_SECRET,
        sealed(&key),
    );
    let mut incomplete = sealed(&key);
    incomplete.pop();
    let b = store(
        directory.path(),
        "store-b",
        "admin-b",
        &STORE_B_SECRET,
        incomplete,
    );
    let missing = directory.path().join("lost");
    let config = config(
        directory.path(),
        &key_path,
        stores(&a, &b, &missing),
        policies(),
    );
    let error = match retrieve(&config) {
        Ok(_) => panic!("incomplete fragment replication unexpectedly passed"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("copy quorum"));
}

#[test]
fn corrupt_encrypted_store_is_excluded_when_two_complete_stores_survive() {
    let directory = tempdir().unwrap();
    let (key_path, key) = recovery_key(directory.path());
    let a = store(
        directory.path(),
        "store-a",
        "admin-a",
        &STORE_A_SECRET,
        sealed(&key),
    );
    let b = store(
        directory.path(),
        "store-b",
        "admin-b",
        &STORE_B_SECRET,
        sealed(&key),
    );
    let mut corrupt = sealed(&key);
    corrupt[0].ciphertext_hex.replace_range(0..2, "00");
    let c = store(
        directory.path(),
        "store-c",
        "admin-c",
        &STORE_C_SECRET,
        corrupt,
    );
    let store_configs = json!([
        {
            "store_id": "store-a", "failure_domain": "admin-a",
            "public_key": hex::encode(SigningKey::from_bytes(&STORE_A_SECRET).verifying_key().to_bytes()),
            "minimum_generation": 7, "transport": {"type": "local_directory", "path": a}
        },
        {
            "store_id": "store-b", "failure_domain": "admin-b",
            "public_key": hex::encode(SigningKey::from_bytes(&STORE_B_SECRET).verifying_key().to_bytes()),
            "minimum_generation": 7, "transport": {"type": "local_directory", "path": b}
        },
        {
            "store_id": "store-c", "failure_domain": "admin-c",
            "public_key": hex::encode(SigningKey::from_bytes(&STORE_C_SECRET).verifying_key().to_bytes()),
            "minimum_generation": 7, "transport": {"type": "local_directory", "path": c}
        }
    ]);
    let config = config(directory.path(), &key_path, store_configs, policies());
    let result = retrieve(&config).unwrap();
    assert_eq!(result.receipt.successful_stores, 2);
    assert_eq!(result.receipt.failed_stores, ["store-c"]);
}

#[test]
fn revoked_key_and_replay_floor_are_rejected() {
    let envelopes = evidence();
    let mut trusted: BTreeMap<String, IssuerPolicy> = serde_json::from_value(policies()).unwrap();
    if let IssuerPolicy::Ed25519(contract) = trusted.get_mut("contract-authority").unwrap() {
        contract.keys[0].revoked_at = Some("2026-08-10T00:00:00+00:00".into());
    }
    assert!(
        collect(envelopes.clone(), &trusted, 2, "turnstile", "1")
            .unwrap_err()
            .to_string()
            .contains("revoked")
    );
    if let IssuerPolicy::Ed25519(contract) = trusted.get_mut("contract-authority").unwrap() {
        contract.keys[0].revoked_at = None;
        contract.minimum_sequence = 1;
    }
    assert!(
        collect(envelopes, &trusted, 2, "turnstile", "1")
            .unwrap_err()
            .to_string()
            .contains("replay floor")
    );
}

#[test]
fn expired_key_is_rejected_at_registered_verification_time() {
    let trusted: BTreeMap<String, IssuerPolicy> = serde_json::from_value(policies()).unwrap();
    let error = collect_at(
        evidence(),
        &trusted,
        2,
        "turnstile",
        "1",
        Some("2028-01-01T00:00:00+00:00"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("verification validity"));
}

#[test]
fn rotation_accepts_distinct_valid_keys_and_equivocation_refuses() {
    let rotated_secret = [0x33; 32];
    let mut trusted: BTreeMap<String, IssuerPolicy> = serde_json::from_value(policies()).unwrap();
    let IssuerPolicy::Ed25519(contract) = trusted.get_mut("contract-authority").unwrap() else {
        panic!("expected production policy");
    };
    contract.keys.push(Ed25519KeyPolicy {
        key_id: "contract-2027".into(),
        public_key: hex::encode(
            SigningKey::from_bytes(&rotated_secret)
                .verifying_key()
                .to_bytes(),
        ),
        not_before: "2026-01-01T00:00:00+00:00".into(),
        not_after: "2027-01-01T00:00:00+00:00".into(),
        revoked_at: None,
    });
    let mut envelopes = evidence();
    envelopes[3] = envelope(
        FragmentKind::Contract,
        "contract-authority",
        "issuer-domain-a",
        3,
        transitions().remove(3),
        "contract-2027",
        &rotated_secret,
    );
    assert_eq!(
        collect(envelopes.clone(), &trusted, 2, "turnstile", "1")
            .unwrap()
            .audit
            .iter()
            .filter(|event| event.key_id == "contract-2027")
            .count(),
        1
    );
    envelopes.push(envelope(
        FragmentKind::Contract,
        "contract-authority",
        "issuer-domain-a",
        3,
        transitions().remove(2),
        "contract-2027",
        &rotated_secret,
    ));
    assert!(
        collect(envelopes, &trusted, 2, "turnstile", "1")
            .unwrap_err()
            .to_string()
            .contains("equivocation")
    );
}

#[test]
fn tampered_and_expired_sealed_evidence_refuses() {
    let directory = tempdir().unwrap();
    let (_, key) = recovery_key(directory.path());
    let mut sealed = sealed(&key).remove(0);
    sealed.ciphertext_hex.replace_range(0..2, "00");
    let keys = BTreeMap::from([(key.key_id.clone(), key.clone())]);
    assert!(
        anasemble::evidence_plane::unseal(&sealed, &keys, "2026-08-12T00:00:00+00:00").is_err()
    );
    let expired = seal(
        &evidence()[0],
        &key,
        "2026-08-01T00:00:00+00:00",
        "2026-08-10T00:00:00+00:00",
    )
    .unwrap();
    assert!(
        anasemble::evidence_plane::unseal(&expired, &keys, "2026-08-12T00:00:00+00:00")
            .unwrap_err()
            .to_string()
            .contains("retention")
    );
}

#[test]
fn remote_transport_requires_https() {
    let directory = tempdir().unwrap();
    let (key_path, _) = recovery_key(directory.path());
    let config = config(
        directory.path(),
        &key_path,
        json!([{
            "store_id": "remote",
            "failure_domain": "admin-remote",
            "public_key": "66".repeat(32),
            "minimum_generation": 1,
            "transport": {"type": "https_bundle", "url": "http://example.invalid/bundle"}
        }]),
        policies(),
    );
    let mut value: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["required_stores"] = Value::from(1);
    write_json(&config, &value);
    assert!(retrieve(&config).is_err());
}
