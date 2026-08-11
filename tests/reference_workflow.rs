mod common;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anasemble::activation::{
    ApprovalPolicy, HealthProbe, IsolationPolicy, KubernetesDeploymentSpec, KubernetesOrchestrator,
    OperatorApproval, RegistryReceipt, approval_payload,
};
use anasemble::fragments::{read_signing_key, sign_detached_with_key_file};
use anasemble::stateful::{ActivationPlan, ActivationStateBinding, BackendKind, S3Adapter};
use postgres::{Client, NoTls};
use redis::Commands;
use tempfile::tempdir;

use common::{build_workspace, write_json};

const POSTGRES_IMAGE: &str =
    "postgres@sha256:9a8afca54e7861fd90fab5fdf4c42477a6b1cb7d293595148e674e0a3181de15";
const MINIO_IMAGE: &str =
    "quay.io/minio/minio@sha256:14cea493d9a34af32f524e538b8346cf79f3321eff8e708c1e2960462bd8936e";
const MC_IMAGE: &str =
    "minio/mc@sha256:a7fe349ef4bd8521fb8497f55c6042871b2ae640607cf99d9bede5e9bdf11727";
const REDIS_IMAGE: &str =
    "redis@sha256:9d317178eceac8454a2284a9e6df2466b93c745529947f0cd42a0fa9609d7005";
const REGISTRY_IMAGE: &str =
    "registry@sha256:a3d8aaa63ed8681a604f1dea0aa03f100d5895b6a58ace528858a7b332415373";
const BASE_IMAGE: &str =
    "debian@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818";
const KUBE_IMAGE: &str = "docker.io/library/debian@sha256:9b67294679b30e5d6ab257b40594feeb4a4b81f7fcf4131f4decf0d6a212a9b0";
const KIND_NODE_IMAGE: &str =
    "sha256:3489c7674813ba5d8b1a9977baea8a6e553784dab7b84759d1014dbd78f7ebd5";

struct Cleanup {
    containers: Vec<String>,
    images: Vec<String>,
    cluster: String,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        for container in &self.containers {
            assert!(container.starts_with("anasemble-reference-"));
            let _ = Command::new("docker")
                .args(["rm", "--force", container])
                .output();
        }
        for image in &self.images {
            assert!(image.contains("anasemble-reference"));
            let _ = Command::new("docker")
                .args(["image", "rm", "--force", image])
                .output();
        }
        assert!(self.cluster.starts_with("anasemble-reference-"));
        let _ = Command::new("kind")
            .args(["delete", "cluster", "--name", &self.cluster])
            .output();
    }
}

struct Container {
    name: String,
}

impl Container {
    fn start(name: String, arguments: &[&str]) -> Self {
        let status = Command::new("docker")
            .args(["run", "--detach", "--name", &name])
            .args(arguments)
            .status()
            .unwrap();
        assert!(status.success());
        Self { name }
    }

    fn port(&self, private: &str) -> u16 {
        retry(|| {
            let output = Command::new("docker")
                .args(["port", &self.name, private])
                .output()
                .ok()?;
            output.status.success().then(|| {
                String::from_utf8(output.stdout)
                    .unwrap()
                    .trim()
                    .rsplit(':')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap()
            })
        })
    }
}

#[test]
fn public_reference_workflow_recovers_activates_and_rolls_back_every_boundary() {
    let suffix = suffix();
    let postgres_name = format!("anasemble-reference-postgres-{suffix}");
    let minio_name = format!("anasemble-reference-minio-{suffix}");
    let redis_name = format!("anasemble-reference-redis-{suffix}");
    let registry_name = format!("anasemble-reference-registry-{suffix}");
    let cluster = format!("anasemble-reference-{suffix}");
    let local_image = format!("anasemble-reference-source:{suffix}");
    let mut cleanup = Cleanup {
        containers: vec![
            postgres_name.clone(),
            minio_name.clone(),
            redis_name.clone(),
            registry_name.clone(),
        ],
        images: vec![local_image.clone()],
        cluster: cluster.clone(),
    };

    let postgres_container = Container::start(
        postgres_name,
        &[
            "--env",
            "POSTGRES_PASSWORD=reference-password",
            "--publish",
            "127.0.0.1::5432",
            POSTGRES_IMAGE,
        ],
    );
    let postgres_url = format!(
        "host=127.0.0.1 port={} user=postgres password=reference-password dbname=postgres",
        postgres_container.port("5432/tcp")
    );
    let mut postgres = retry(|| Client::connect(&postgres_url, NoTls).ok());
    postgres
        .batch_execute(
            "CREATE SCHEMA source_state;
             CREATE TABLE source_state.accounts(id bigint PRIMARY KEY, name text NOT NULL UNIQUE);
             INSERT INTO source_state.accounts VALUES (1,'Ada'),(2,'Grace');
             CREATE SCHEMA active_state;
             CREATE TABLE active_state.accounts(id bigint PRIMARY KEY, name text NOT NULL);
             INSERT INTO active_state.accounts VALUES (99,'Previous');",
        )
        .unwrap();

    let minio_container = Container::start(
        minio_name,
        &[
            "--env",
            "MINIO_ROOT_USER=reference-access",
            "--env",
            "MINIO_ROOT_PASSWORD=reference-secret",
            "--publish",
            "127.0.0.1::9000",
            MINIO_IMAGE,
            "server",
            "/data",
        ],
    );
    let minio_endpoint = format!("http://127.0.0.1:{}", minio_container.port("9000/tcp"));
    let bucket = format!("reference-{suffix}");
    retry(|| {
        Command::new("docker")
            .args([
                "run",
                "--rm",
                "--network",
                &format!("container:{}", minio_container.name),
                "--env",
                "MC_HOST_local=http://reference-access:reference-secret@127.0.0.1:9000",
                MC_IMAGE,
                "mb",
                &format!("local/{bucket}"),
            ])
            .status()
            .ok()
            .filter(|status| status.success())
    });
    let s3 = S3Adapter::connect(
        &minio_endpoint,
        "us-east-1",
        &bucket,
        "reference-access",
        "reference-secret",
        "source/",
    )
    .unwrap();
    s3.put_object("source/avatar.bin", b"recovered-avatar")
        .unwrap();
    s3.put_object("active/old.bin", b"previous-object").unwrap();

    let redis_container = Container::start(
        redis_name,
        &[
            "--publish",
            "127.0.0.1::6379",
            REDIS_IMAGE,
            "redis-server",
            "--appendonly",
            "yes",
        ],
    );
    let redis_url = format!("redis://127.0.0.1:{}/", redis_container.port("6379/tcp"));
    let redis_client = redis::Client::open(redis_url.clone()).unwrap();
    let mut redis = retry(|| redis_client.get_connection().ok());
    redis::cmd("XADD")
        .arg("source-events")
        .arg("1000-0")
        .arg("event")
        .arg("created")
        .query::<String>(&mut redis)
        .unwrap();
    redis::cmd("XGROUP")
        .arg("CREATE")
        .arg("source-events")
        .arg("workers")
        .arg("1000-0")
        .query::<()>(&mut redis)
        .unwrap();
    redis::cmd("XADD")
        .arg("active-events")
        .arg("1-0")
        .arg("event")
        .arg("previous")
        .query::<String>(&mut redis)
        .unwrap();

    let registry_container =
        Container::start(registry_name, &["--network", "host", REGISTRY_IMAGE]);
    retry(|| std::net::TcpStream::connect(("127.0.0.1", 5000)).ok());
    let registry_endpoint = "127.0.0.1:5000".to_string();
    cleanup
        .images
        .push(format!("{registry_endpoint}/anasemble-reference:{suffix}"));

    assert!(
        Command::new("kind")
            .args([
                "create",
                "cluster",
                "--name",
                &cluster,
                "--image",
                KIND_NODE_IMAGE,
                "--wait",
                "60s",
            ])
            .status()
            .unwrap()
            .success()
    );
    load_debian_into_kind(&format!("{cluster}-control-plane"));
    let context = format!("kind-{cluster}");
    let namespace = format!("reference-{}", std::process::id());
    kubectl(&context, &["create", "namespace", &namespace]);

    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    assert!(!workspace.artifact.exists());
    assert_eq!(workspace.artifact_digest.len(), 64);
    let service = service_manifest();
    let service_path = directory.path().join("service.json");
    write_json(&service_path, &service);
    let registry_path = workspace.recovery.join("registry.json");
    let mut recovery_registry: serde_json::Value =
        serde_json::from_slice(&fs::read(&registry_path).unwrap()).unwrap();
    recovery_registry["service_manifest"] = service;
    write_json(&registry_path, &recovery_registry);

    let key_path = directory.path().join("operator-key.json");
    anasemble::fragments::create_signing_key(
        &key_path,
        "reference-operator",
        "2026-08-11T00:00:00+00:00",
    )
    .unwrap();
    let key = read_signing_key(&key_path).unwrap();
    seed_prior_kubernetes(&context, &namespace, &key);

    let postgres_file = secret_file(directory.path(), "postgres.txt", &postgres_url);
    let s3_access = secret_file(directory.path(), "s3-access.txt", "reference-access");
    let s3_secret = secret_file(directory.path(), "s3-secret.txt", "reference-secret");
    let redis_file = secret_file(directory.path(), "redis.txt", &redis_url);
    let config_path = directory.path().join("reference-config.json");
    write_json(
        &config_path,
        &serde_json::json!({
            "version":"reference-recovery-config-v1",
            "workspace":workspace.recovery,
            "service_manifest":service_path,
            "postgres":{"connection_file":postgres_file,"source_schema":"source_state","target_schema":"active_state"},
            "s3":{"endpoint":minio_endpoint,"region":"us-east-1","bucket":bucket,"access_key_file":s3_access,"secret_key_file":s3_secret,"source_prefix":"source/","target_prefix":"active/"},
            "redis":{"url_file":redis_file,"source_stream":"source-events","target_stream":"active-events"},
            "artifact":{"base_image":BASE_IMAGE,"local_image":local_image,"registry_endpoint":registry_endpoint,"repository":"anasemble-reference","tag":suffix,"kind_import":{"node":format!("{cluster}-control-plane"),"platform":"linux/arm64"}},
            "kubernetes":{"context":context,"namespace":namespace,"service":"turnstile","command":["/bin/sleep","300"],"health_command":["/usr/bin/test","-s","/candidate.json"],"health_attempts":30,"health_interval_ms":1000,"service_port":8080,"container_port":8080,"secrets":[]},
            "operator":{"signing_key":key_path,"not_before":"2026-08-11T00:00:00+00:00","not_after":"2026-08-12T00:00:00+00:00","approved_at":"2026-08-11T12:00:00+00:00"},
            "isolation":policy()
        }),
    );
    let bundle = directory.path().join("state-bundle.json");
    command(&[
        "prepare-reference-recovery",
        path(&config_path),
        path(&bundle),
    ]);
    assert_eq!(
        fs::metadata(&bundle).unwrap().permissions().mode() & 0o077,
        0
    );

    postgres
        .batch_execute("DROP SCHEMA source_state CASCADE")
        .unwrap();
    redis::cmd("DEL")
        .arg("source-events")
        .query::<i64>(&mut redis)
        .unwrap();
    remove_minio_source(&minio_container.name, &bucket);

    let receipt_path = directory.path().join("recovery-receipt.json");
    let recovery_arguments = [
        "recover-activate-reference",
        path(&config_path),
        path(&bundle),
        path(&receipt_path),
    ];
    let recovery_output = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .args(recovery_arguments)
        .output()
        .unwrap();
    if !recovery_output.status.success() {
        eprintln!(
            "{}",
            kubectl(&context, &["get", "pods", "-n", &namespace, "-o", "wide"])
        );
        eprintln!(
            "{}",
            kubectl(&context, &["describe", "pods", "-n", &namespace])
        );
        let images = Command::new("docker")
            .args([
                "exec",
                &format!("{cluster}-control-plane"),
                "ctr",
                "--namespace=k8s.io",
                "images",
                "list",
            ])
            .output()
            .unwrap();
        eprintln!("{}", String::from_utf8_lossy(&images.stdout));
    }
    assert!(
        recovery_output.status.success(),
        "{}",
        String::from_utf8_lossy(&recovery_output.stderr)
    );
    let recovered: serde_json::Value = serde_json::from_slice(&recovery_output.stdout).unwrap();
    assert_eq!(recovered["rollback_available"], true);
    assert_eq!(
        fs::metadata(&receipt_path).unwrap().permissions().mode() & 0o077,
        0
    );
    let recovered_rows: i64 = postgres
        .query_one("SELECT count(*) FROM active_state.accounts", &[])
        .unwrap()
        .get(0);
    assert_eq!(recovered_rows, 2);
    assert_eq!(
        s3.get_object("active/avatar.bin").unwrap(),
        b"recovered-avatar"
    );
    assert_eq!(redis.xlen::<_, i64>("active-events").unwrap(), 1);
    let new_selector = kube_selector(&context, &namespace);
    assert_eq!(
        new_selector,
        recovered["plan_sha256"].as_str().unwrap()[..63]
    );

    let tampered_receipt = directory.path().join("tampered-recovery-receipt.json");
    let mut tampered: serde_json::Value =
        serde_json::from_slice(&fs::read(&receipt_path).unwrap()).unwrap();
    tampered["redis"]["target_resource"] = "unrelated-events".into();
    write_json(&tampered_receipt, &tampered);
    fs::set_permissions(&tampered_receipt, fs::Permissions::from_mode(0o600)).unwrap();
    let refused = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .args([
            "rollback-reference-recovery",
            path(&config_path),
            path(&tampered_receipt),
        ])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("receipt digest is invalid"));
    assert_eq!(redis.xlen::<_, i64>("active-events").unwrap(), 1);
    assert_eq!(kube_selector(&context, &namespace), new_selector);

    command(&[
        "rollback-reference-recovery",
        path(&config_path),
        path(&receipt_path),
    ]);
    let previous: String = postgres
        .query_one("SELECT name FROM active_state.accounts", &[])
        .unwrap()
        .get(0);
    assert_eq!(previous, "Previous");
    assert_eq!(s3.get_object("active/old.bin").unwrap(), b"previous-object");
    assert_eq!(redis.xlen::<_, i64>("active-events").unwrap(), 1);
    assert_eq!(
        kube_selector(&context, &namespace),
        prior_plan().plan_sha256[..63]
    );

    kubectl(
        &context,
        &[
            "delete",
            "deployment",
            &format!(
                "turnstile-stage-{}",
                &recovered["plan_sha256"].as_str().unwrap()[..12]
            ),
            "-n",
            &namespace,
            "--wait=true",
            "--timeout=30s",
        ],
    );
    assert!(
        Command::new("docker")
            .args(["image", "rm", &local_image])
            .status()
            .unwrap()
            .success()
    );
    let accepted_receipt = directory.path().join("accepted-recovery-receipt.json");
    command(&[
        "recover-activate-reference",
        path(&config_path),
        path(&bundle),
        path(&accepted_receipt),
    ]);
    command(&[
        "commit-reference-recovery",
        path(&config_path),
        path(&accepted_receipt),
    ]);
    let rollback_after_commit = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .args([
            "rollback-reference-recovery",
            path(&config_path),
            path(&accepted_receipt),
        ])
        .output()
        .unwrap();
    assert!(!rollback_after_commit.status.success());
    assert_eq!(redis.xlen::<_, i64>("active-events").unwrap(), 1);
    assert_eq!(
        kube_selector(&context, &namespace),
        recovered["plan_sha256"].as_str().unwrap()[..63]
    );

    cleanup.containers.clear();
    drop(cleanup);
    for name in [
        postgres_container.name,
        minio_container.name,
        redis_container.name,
        registry_container.name,
    ] {
        let _ = Command::new("docker")
            .args(["rm", "--force", &name])
            .output();
    }
}

fn service_manifest() -> serde_json::Value {
    serde_json::json!({"version":"service-v1","component":"turnstile","interface_version":"1","http":{"endpoints":[{"method":"POST","path":"/transition","request_schema_sha256":"11".repeat(32),"response_schema_sha256":"22".repeat(32)}]},"effects":[{"kind":"state","target":"database","access":"read_write"},{"kind":"state","target":"objects","access":"read_write"},{"kind":"state","target":"events","access":"read_write"}],"state_dependencies":[{"name":"database","adapter":"postgres","consistency":"transactional","required":true},{"name":"objects","adapter":"object_store","consistency":"snapshot","required":true},{"name":"events","adapter":"queue","consistency":"snapshot","required":true}],"limits":{"request_bytes":4096,"response_bytes":4096,"wall_time_ms":1000,"concurrent_requests":8}})
}

fn policy() -> IsolationPolicy {
    IsolationPolicy {
        cpu_millis: 250,
        memory_bytes: 64 * 1024 * 1024,
        pids: 16,
        wall_time_ms: 10_000,
        output_bytes: 65_536,
        writable_tmpfs_bytes: 4 * 1024 * 1024,
        linux_capabilities: Vec::new(),
        network_egress_allowlist: Vec::new(),
    }
}

fn prior_plan() -> ActivationPlan {
    let value = anasemble::canonical::bytes_digest(b"reference-prior-plan");
    let mut plan = ActivationPlan {
        version: "activation-plan-v1".into(),
        candidate_sha256: value.clone(),
        certificate_sha256: value.clone(),
        service_manifest_sha256: value.clone(),
        states: vec![ActivationStateBinding {
            backend: BackendKind::PostgreSql,
            resource: "active_state".into(),
            schema_sha256: value.clone(),
            snapshot_sha256: value.clone(),
            migration_sha256: value,
        }],
        plan_sha256: String::new(),
    };
    plan.plan_sha256 = anasemble::canonical::digest(&plan).unwrap();
    plan
}

fn seed_prior_kubernetes(
    context: &str,
    namespace: &str,
    key: &anasemble::fragments::SigningKeyFile,
) {
    let plan = prior_plan();
    let mut artifact = RegistryReceipt {
        repository: "docker.io/library/debian".into(),
        immutable_image: KUBE_IMAGE.into(),
        manifest_sha256: KUBE_IMAGE.rsplit_once("sha256:").unwrap().1.into(),
        activation_plan_sha256: plan.plan_sha256.clone(),
        candidate_sha256: plan.candidate_sha256.clone(),
        binding_sha256: String::new(),
    };
    artifact.binding_sha256 = anasemble::canonical::digest(&artifact).unwrap();
    let public_key: [u8; 32] = hex::decode(&key.public_key_hex)
        .unwrap()
        .try_into()
        .unwrap();
    let orchestrator = KubernetesOrchestrator::new(
        context,
        ApprovalPolicy {
            operator_keys: BTreeMap::from([(key.key_id.clone(), public_key)]),
            not_before: "2026-08-11T00:00:00+00:00".into(),
            not_after: "2026-08-12T00:00:00+00:00".into(),
        },
    )
    .unwrap();
    let spec = KubernetesDeploymentSpec {
        version: "kubernetes-deployment-v1".into(),
        namespace: namespace.into(),
        service: "turnstile".into(),
        artifact: artifact.clone(),
        command: vec!["/bin/sleep".into(), "300".into()],
        isolation: policy(),
        secrets: Vec::new(),
        health: HealthProbe {
            command: vec!["/bin/true".into()],
            attempts: 30,
            interval_ms: 1_000,
        },
        service_port: 8080,
        container_port: 8080,
    };
    let mut approval = OperatorApproval {
        version: "operator-approval-v1".into(),
        plan_sha256: plan.plan_sha256.clone(),
        artifact_sha256: artifact.binding_sha256.clone(),
        operator_key_id: key.key_id.clone(),
        approved_at: "2026-08-11T12:00:00+00:00".into(),
        signature: String::new(),
    };
    approval.signature =
        sign_detached_with_key_file(&approval_payload(&approval).unwrap(), key).unwrap();
    orchestrator.activate(&spec, &approval).unwrap();
    orchestrator.commit(namespace, "turnstile").unwrap();
}

fn load_debian_into_kind(node: &str) {
    let mut save = Command::new("docker")
        .args(["save", "debian:bookworm-slim"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let input = save.stdout.take().unwrap();
    assert!(
        Command::new("docker")
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
            .unwrap()
            .success()
    );
    assert!(save.wait().unwrap().success());
}

fn remove_minio_source(container: &str, bucket: &str) {
    assert!(
        Command::new("docker")
            .args([
                "run",
                "--rm",
                "--network",
                &format!("container:{container}"),
                "--env",
                "MC_HOST_local=http://reference-access:reference-secret@127.0.0.1:9000",
                MC_IMAGE,
                "rm",
                "--recursive",
                "--force",
                &format!("local/{bucket}/source"),
            ])
            .status()
            .unwrap()
            .success()
    );
}

fn secret_file(root: &std::path::Path, name: &str, value: &str) -> std::path::PathBuf {
    let path = root.join(name);
    fs::write(&path, value).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    path
}

fn kubectl(context: &str, arguments: &[&str]) -> String {
    let output = Command::new("kubectl")
        .args(["--context", context])
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn kube_selector(context: &str, namespace: &str) -> String {
    kubectl(
        context,
        &[
            "get",
            "service",
            "turnstile",
            "-n",
            namespace,
            "-o",
            "jsonpath={.spec.selector.anasemble\\.plan}",
        ],
    )
}

fn command(arguments: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_anasemble"))
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{:?}: {}",
        arguments,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn retry<T>(mut operation: impl FnMut() -> Option<T>) -> T {
    for _ in 0..120 {
        if let Some(value) = operation() {
            return value;
        }
        thread::sleep(Duration::from_millis(250));
    }
    panic!("backend did not become ready within 30 seconds");
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

fn path(value: &std::path::Path) -> &str {
    value.to_str().unwrap()
}
