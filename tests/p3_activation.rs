use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

use anasemble::activation::{
    ActivationFailurePoint, ApprovalPolicy, DeploymentSpec, DockerOrchestrator, DockerRegistry,
    DockerSandbox, HealthProbe, IsolationPolicy, KubernetesDeploymentSpec, KubernetesOrchestrator,
    KubernetesSecretReference, OperatorApproval, RegistryReceipt, SandboxRequest, SecretReference,
    approval_payload,
};
use anasemble::stateful::{ActivationPlan, ActivationStateBinding, BackendKind};
use ed25519_dalek::{Signer, SigningKey};
use tempfile::Builder;

struct DockerCleanup {
    containers: Vec<String>,
    images: Vec<String>,
    networks: Vec<String>,
}

struct KindCleanup {
    name: String,
}
impl Drop for KindCleanup {
    fn drop(&mut self) {
        assert!(self.name.starts_with("anasemble-p3-"));
        let _ = Command::new("kind")
            .args(["delete", "cluster", "--name", &self.name])
            .output();
    }
}

impl Drop for DockerCleanup {
    fn drop(&mut self) {
        for container in &self.containers {
            assert!(container.starts_with("anasemble-"));
            let _ = Command::new("docker")
                .args(["rm", "--force", container])
                .output();
        }
        for image in &self.images {
            assert!(image.contains("anasemble-p3"));
            let _ = Command::new("docker")
                .args(["image", "rm", "--force", image])
                .output();
        }
        for network in &self.networks {
            assert!(network.starts_with("anasemble-"));
            let _ = Command::new("docker")
                .args(["network", "rm", network])
                .output();
        }
    }
}

fn suffix() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

fn docker(args: &[&str]) -> String {
    let output = Command::new("docker").args(args).output().unwrap();
    assert!(
        output.status.success(),
        "docker {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn immutable_debian() -> String {
    let values: Vec<String> = serde_json::from_str(&docker(&[
        "image",
        "inspect",
        "--format",
        "{{json .RepoDigests}}",
        "debian:bookworm-slim",
    ]))
    .unwrap();
    let value = values.into_iter().next().unwrap();
    if value.starts_with("debian@") {
        format!("docker.io/library/{value}")
    } else {
        value
    }
}

fn policy() -> IsolationPolicy {
    IsolationPolicy {
        cpu_millis: 250,
        memory_bytes: 64 * 1024 * 1024,
        pids: 16,
        wall_time_ms: 5_000,
        output_bytes: 65_536,
        writable_tmpfs_bytes: 4 * 1024 * 1024,
        linux_capabilities: Vec::new(),
        network_egress_allowlist: Vec::new(),
    }
}

fn signed_approval(key: &SigningKey, plan: &str, artifact: &str) -> OperatorApproval {
    let mut approval = OperatorApproval {
        version: "operator-approval-v1".into(),
        plan_sha256: plan.into(),
        artifact_sha256: artifact.into(),
        operator_key_id: "operator-a".into(),
        approved_at: "2026-08-11T12:00:00Z".into(),
        signature: String::new(),
    };
    approval.signature = hex::encode(key.sign(&approval_payload(&approval).unwrap()).to_bytes());
    approval
}

fn activation_plan(seed: &str) -> ActivationPlan {
    let digest = anasemble::canonical::bytes_digest(seed.as_bytes());
    let mut plan = ActivationPlan {
        version: "activation-plan-v1".into(),
        candidate_sha256: digest.clone(),
        certificate_sha256: digest.clone(),
        service_manifest_sha256: digest.clone(),
        states: vec![ActivationStateBinding {
            backend: BackendKind::PostgreSql,
            resource: "database".into(),
            schema_sha256: digest.clone(),
            snapshot_sha256: digest.clone(),
            migration_sha256: digest,
        }],
        plan_sha256: String::new(),
    };
    plan.plan_sha256 = anasemble::canonical::digest(&plan).unwrap();
    plan
}

fn local_artifact(plan: &ActivationPlan) -> RegistryReceipt {
    let image = kube_image();
    let manifest_sha256 = image.rsplit_once("@sha256:").unwrap().1.to_string();
    let mut receipt = RegistryReceipt {
        repository: "docker.io/library/debian".into(),
        immutable_image: image,
        manifest_sha256,
        activation_plan_sha256: plan.plan_sha256.clone(),
        candidate_sha256: plan.candidate_sha256.clone(),
        binding_sha256: String::new(),
    };
    receipt.binding_sha256 = anasemble::canonical::digest(&receipt).unwrap();
    receipt
}

fn kube_image() -> String {
    "docker.io/library/debian@sha256:9b67294679b30e5d6ab257b40594feeb4a4b81f7fcf4131f4decf0d6a212a9b0".into()
}

#[test]
fn docker_sandbox_enforces_supported_os_capability_boundary() {
    let image = immutable_debian();
    let receipt = DockerSandbox::run(&SandboxRequest {
        image: image.clone(),
        command: vec![
            "/bin/sh".into(),
            "-c".into(),
            "set -eu; test \"$(cat /sys/fs/cgroup/pids.max)\" = 16; test \"$(awk '/CapEff/{print $2}' /proc/self/status)\" = 0000000000000000; test \"$(wc -l < /proc/net/route)\" = 1; ! touch /root/forbidden; touch /tmp/allowed; echo isolated".into(),
        ],
        policy: policy(),
    })
    .unwrap();
    assert_eq!(receipt.exit_code, 0);
    assert_eq!(receipt.stdout, b"isolated\n");
    assert!(!receipt.timed_out);

    let mut timeout_policy = policy();
    timeout_policy.wall_time_ms = 100;
    let timeout = DockerSandbox::run(&SandboxRequest {
        image,
        command: vec!["/bin/sleep".into(), "10".into()],
        policy: timeout_policy,
    })
    .unwrap();
    assert!(timeout.timed_out);

    let mut unsupported = policy();
    unsupported.network_egress_allowlist = vec!["203.0.113.10/32".into()];
    assert!(
        DockerSandbox::run(&SandboxRequest {
            image: immutable_debian(),
            command: vec!["/bin/true".into()],
            policy: unsupported,
        })
        .is_err()
    );
}

#[test]
fn registry_activation_reconciles_interruption_refuses_split_brain_and_rolls_back() {
    let suffix = suffix();
    let registry_name = format!("anasemble-p3-registry-{suffix}");
    let registry_port = docker(&[
        "run",
        "--detach",
        "--name",
        &registry_name,
        "--network",
        "host",
        "registry:2",
    ]);
    assert!(!registry_port.is_empty());
    let endpoint = "127.0.0.1:5000".to_string();
    let tagged = format!("{endpoint}/anasemble-p3:{suffix}");
    let source = format!("anasemble-p3-source:{suffix}");
    let package = format!("anasemble-p3-package-{suffix}");
    let mut cleanup = DockerCleanup {
        containers: vec![registry_name.clone()],
        images: vec![tagged, source.clone()],
        networks: Vec::new(),
    };
    let registry = DockerRegistry::connect(&endpoint).unwrap();
    let primary_plan = activation_plan("primary-plan");
    docker(&[
        "create",
        "--name",
        &package,
        "--label",
        &format!("anasemble.plan={}", primary_plan.plan_sha256),
        "--label",
        &format!("anasemble.candidate={}", primary_plan.candidate_sha256),
        "debian:bookworm-slim",
        "/bin/true",
    ]);
    cleanup.containers.push(package.clone());
    docker(&["commit", &package, &source]);
    remove_container(&package);
    cleanup.containers.retain(|name| name != &package);
    let artifact = registry
        .publish(&source, "anasemble-p3", &suffix, &primary_plan)
        .unwrap();
    assert_eq!(artifact.manifest_sha256.len(), 64);

    let service = format!("p3svc{}", std::process::id());
    let active = format!("anasemble-{service}-active");
    let stage = format!(
        "anasemble-{service}-stage-{}",
        &primary_plan.plan_sha256[..12]
    );
    let rollback = format!("anasemble-{service}-rollback");
    let failed = format!("anasemble-{service}-failed");
    let lease = format!("anasemble-{service}-lease");
    let network = format!("anasemble-{service}-network");
    cleanup.networks.push(network.clone());
    cleanup.containers.extend([
        active.clone(),
        stage,
        rollback.clone(),
        failed,
        lease.clone(),
    ]);
    docker(&[
        "create",
        "--name",
        &active,
        "--label",
        &format!("anasemble.plan={}", "11".repeat(32)),
        &artifact.immutable_image,
        "/bin/sleep",
        "300",
    ]);
    docker(&["start", &active]);

    let secret_dir = Builder::new()
        .prefix("p3-secret-")
        .tempdir_in("target")
        .unwrap();
    let secret_file = secret_dir.path().join("token");
    fs::write(&secret_file, b"never-in-receipts-or-logs").unwrap();
    fs::set_permissions(&secret_file, fs::Permissions::from_mode(0o600)).unwrap();

    let plan = primary_plan.plan_sha256.clone();
    let key = SigningKey::from_bytes(&[0x71; 32]);
    let orchestrator = DockerOrchestrator::new(ApprovalPolicy {
        operator_keys: BTreeMap::from([("operator-a".into(), key.verifying_key().to_bytes())]),
        not_before: "2026-08-11T00:00:00Z".into(),
        not_after: "2026-08-12T00:00:00Z".into(),
    });
    let spec = DeploymentSpec {
        version: "docker-deployment-v1".into(),
        service: service.clone(),
        artifact: artifact.clone(),
        command: vec!["/bin/sleep".into(), "300".into()],
        isolation: policy(),
        secrets: vec![SecretReference {
            id: "service-token".into(),
            source_file: secret_file,
            mount_path: "/run/secrets/service-token".into(),
        }],
        health: HealthProbe {
            command: vec![
                "/bin/sh".into(),
                "-c".into(),
                "test -s /run/secrets/service-token && test ! -w /".into(),
            ],
            attempts: 20,
            interval_ms: 50,
        },
    };
    let approval = signed_approval(&key, &plan, &artifact.binding_sha256);
    assert!(
        orchestrator
            .activate_with_failure(&spec, &approval, ActivationFailurePoint::AfterActiveRenamed)
            .is_err()
    );
    assert!(
        !Command::new("docker")
            .args(["container", "inspect", &active])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(
        docker(&["inspect", "--format", "{{.State.Running}}", &rollback]),
        "false"
    );
    assert!(docker(&["container", "inspect", &lease]).contains(&lease));

    let other_plan = activation_plan("other-plan");
    let mut other_spec = spec.clone();
    other_spec.artifact.activation_plan_sha256 = other_plan.plan_sha256.clone();
    other_spec.artifact.candidate_sha256 = other_plan.candidate_sha256;
    other_spec.artifact.binding_sha256.clear();
    other_spec.artifact.binding_sha256 =
        anasemble::canonical::digest(&other_spec.artifact).unwrap();
    assert!(
        orchestrator
            .activate(
                &other_spec,
                &signed_approval(
                    &key,
                    &other_plan.plan_sha256,
                    &other_spec.artifact.binding_sha256
                )
            )
            .is_err()
    );
    let receipt = orchestrator.activate(&spec, &approval).unwrap();
    assert!(receipt.rollback_available);
    assert_eq!(receipt.secret_reference_ids, ["service-token"]);
    let serialized = serde_json::to_string(&receipt).unwrap();
    assert!(!serialized.contains("never-in-receipts-or-logs"));
    assert_eq!(
        docker(&["inspect", "--format", "{{.State.Running}}", &active]),
        "true"
    );
    assert_eq!(
        docker(&[
            "inspect",
            "--format",
            "{{.HostConfig.NetworkMode}}",
            &active
        ]),
        network
    );
    assert_eq!(
        docker(&[
            "inspect",
            "--format",
            "{{.HostConfig.ReadonlyRootfs}}",
            &active
        ]),
        "true"
    );
    assert_eq!(
        docker(&[
            "inspect",
            "--format",
            "{{.HostConfig.LogConfig.Type}}",
            &active
        ]),
        "none"
    );
    let egress = Command::new("docker")
        .args([
            "exec",
            &active,
            "/bin/bash",
            "-c",
            "timeout 1 bash -c '</dev/tcp/1.1.1.1/53'",
        ])
        .output()
        .unwrap();
    assert!(!egress.status.success());
    let idempotent = orchestrator.activate(&spec, &approval).unwrap();
    assert!(idempotent.idempotent);
    orchestrator.rollback(&service, &plan).unwrap();
    assert_eq!(
        docker(&[
            "inspect",
            "--format",
            "{{index .Config.Labels \"anasemble.plan\"}}",
            &active
        ]),
        "11".repeat(32)
    );
    assert!(orchestrator.commit(&service, &plan).is_err());

    remove_container(&active);
    cleanup.containers.retain(|name| name != &active);
}

fn remove_container(name: &str) {
    let output = Command::new("docker")
        .args(["rm", "--force", name])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn kubernetes_orchestrator_switches_atomically_and_reconciles_the_lease() {
    let cluster = format!("anasemble-p3-{}", std::process::id());
    let guard = KindCleanup {
        name: cluster.clone(),
    };
    let node_image = "sha256:3489c7674813ba5d8b1a9977baea8a6e553784dab7b84759d1014dbd78f7ebd5";
    let status = Command::new("kind")
        .args([
            "create", "cluster", "--name", &cluster, "--image", node_image, "--wait", "60s",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    load_image_into_kind(&format!("{cluster}-control-plane"));
    let context = format!("kind-{cluster}");
    let namespace = format!("p3-{}", std::process::id());
    assert!(
        (Command::new("kubectl")
            .args(["--context", &context, "create", "namespace", &namespace])
            .status()
            .unwrap()
            .success())
    );
    let secret_dir = Builder::new()
        .prefix("p3-kube-secret-")
        .tempdir_in("target")
        .unwrap();
    let secret_file = secret_dir.path().join("token");
    fs::write(&secret_file, b"kubernetes-secret-value").unwrap();
    assert!(
        Command::new("kubectl")
            .args([
                "--context",
                &context,
                "create",
                "secret",
                "generic",
                "service-token",
                "-n",
                &namespace,
                &format!("--from-file=token={}", secret_file.display())
            ])
            .status()
            .unwrap()
            .success()
    );
    let key = SigningKey::from_bytes(&[0x72; 32]);
    let approval_policy = ApprovalPolicy {
        operator_keys: BTreeMap::from([("operator-a".into(), key.verifying_key().to_bytes())]),
        not_before: "2026-08-11T00:00:00Z".into(),
        not_after: "2026-08-12T00:00:00Z".into(),
    };
    let orchestrator = KubernetesOrchestrator::new(&context, approval_policy).unwrap();
    let make_spec = |seed: &str| {
        let plan = activation_plan(seed);
        let artifact = local_artifact(&plan);
        let spec = KubernetesDeploymentSpec {
            version: "kubernetes-deployment-v1".into(),
            namespace: namespace.clone(),
            service: "turnstile".into(),
            artifact: artifact.clone(),
            command: vec!["/bin/sleep".into(), "300".into()],
            isolation: policy(),
            secrets: vec![KubernetesSecretReference {
                id: "token".into(),
                secret_name: "service-token".into(),
                secret_key: "token".into(),
                mount_path: "/run/secrets/token".into(),
            }],
            health: HealthProbe {
                command: vec![
                    "/usr/bin/test".into(),
                    "-s".into(),
                    "/run/secrets/token".into(),
                ],
                attempts: 30,
                interval_ms: 1_000,
            },
            service_port: 8080,
            container_port: 8080,
        };
        (plan, spec)
    };
    let (old_plan, old_spec) = make_spec("kube-old");
    let old_approval = signed_approval(
        &key,
        &old_plan.plan_sha256,
        &old_spec.artifact.binding_sha256,
    );
    orchestrator.activate(&old_spec, &old_approval).unwrap();
    orchestrator.commit(&namespace, "turnstile").unwrap();
    let (new_plan, new_spec) = make_spec("kube-new");
    let new_approval = signed_approval(
        &key,
        &new_plan.plan_sha256,
        &new_spec.artifact.binding_sha256,
    );
    assert!(
        orchestrator
            .activate_with_failure(
                &new_spec,
                &new_approval,
                ActivationFailurePoint::AfterServiceSwitched
            )
            .is_err()
    );
    let selected = docker_kubectl(
        &context,
        &[
            "get",
            "service",
            "turnstile",
            "-n",
            &namespace,
            "-o",
            "jsonpath={.spec.selector.anasemble\\.plan}",
        ],
    );
    assert_eq!(selected, new_plan.plan_sha256[..63]);
    assert!(orchestrator.rollback(&namespace, "turnstile").is_err());
    let (other_plan, other_spec) = make_spec("kube-other");
    assert!(
        orchestrator
            .activate(
                &other_spec,
                &signed_approval(
                    &key,
                    &other_plan.plan_sha256,
                    &other_spec.artifact.binding_sha256
                )
            )
            .is_err()
    );
    let resumed = orchestrator.activate(&new_spec, &new_approval).unwrap();
    assert!(resumed.idempotent);
    orchestrator.rollback(&namespace, "turnstile").unwrap();
    let selected = docker_kubectl(
        &context,
        &[
            "get",
            "service",
            "turnstile",
            "-n",
            &namespace,
            "-o",
            "jsonpath={.spec.selector.anasemble\\.plan}",
        ],
    );
    assert_eq!(selected, old_plan.plan_sha256[..63]);
    let deployment = docker_kubectl(
        &context,
        &[
            "get",
            "deployment",
            &format!("turnstile-stage-{}", &new_plan.plan_sha256[..12]),
            "-n",
            &namespace,
            "-o",
            "json",
        ],
    );
    assert!(!deployment.contains("kubernetes-secret-value"));
    let value: serde_json::Value = serde_json::from_str(&deployment).unwrap();
    assert_eq!(
        value
            .pointer("/spec/template/spec/automountServiceAccountToken")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value
            .pointer("/spec/template/spec/containers/0/securityContext/readOnlyRootFilesystem")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    drop(guard);
}

fn docker_kubectl(context: &str, args: &[&str]) -> String {
    let mut command = Command::new("kubectl");
    command.args(["--context", context]).args(args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn load_image_into_kind(node: &str) {
    let mut save = Command::new("docker")
        .args(["save", "debian:bookworm-slim"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let input = save.stdout.take().unwrap();
    let status = Command::new("docker")
        .args([
            "exec",
            "-i",
            node,
            "ctr",
            "--namespace=k8s.io",
            "images",
            "import",
            "--platform",
            "linux/arm64",
            "--digests",
            "-",
        ])
        .stdin(Stdio::from(input))
        .status()
        .unwrap();
    assert!(status.success());
    assert!(save.wait().unwrap().success());
}
