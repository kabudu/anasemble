use std::collections::BTreeMap;
use std::env;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use anasemble::activation::{
    ApprovalPolicy, HealthProbe, IsolationPolicy, KubernetesDeploymentSpec, KubernetesOrchestrator,
    KubernetesSecretReference, OperatorApproval, RegistryReceipt, approval_payload,
};
use anasemble::stateful::{
    ActivationPlan, ActivationStateBinding, BackendKind, PostgresAdapter, RedisStreamAdapter,
    S3Adapter, TransactionalStateAdapter,
};
use ed25519_dalek::{Signer, SigningKey};
use postgres::Client;
use redis::Commands;
use rustls::pki_types::{CertificateDer, pem::PemObject};
use rustls::{ClientConfig, RootCertStore};
use tokio_postgres_rustls::MakeRustlsConnect;

#[test]
#[ignore = "requires tagged ephemeral AWS fixtures"]
fn aws_remote_state_profiles_restore_and_rollback_over_tls() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let postgres_uri = required("ANASEMBLE_AWS_POSTGRES_URI");
    let postgres_ca = std::fs::read(required("ANASEMBLE_AWS_POSTGRES_CA")).unwrap();
    let redis_url = required("ANASEMBLE_AWS_REDIS_URL");
    let s3_endpoint = required("ANASEMBLE_AWS_S3_ENDPOINT");
    let s3_region = required("ANASEMBLE_AWS_S3_REGION");
    let s3_bucket = required("ANASEMBLE_AWS_S3_BUCKET");
    let s3_access_key = required("ANASEMBLE_AWS_S3_ACCESS_KEY");
    let s3_secret_key = required("ANASEMBLE_AWS_S3_SECRET_KEY");
    let s3_session_token = required("ANASEMBLE_AWS_S3_SESSION_TOKEN");
    let suffix = required("ANASEMBLE_AWS_RUN_SUFFIX");

    let source_schema = format!("source_{suffix}");
    let target_schema = format!("target_{suffix}");
    let mut postgres = postgres_client(&postgres_uri, &postgres_ca);
    postgres
        .batch_execute(&format!(
            "CREATE SCHEMA {source_schema}; CREATE TABLE {source_schema}.records (id bigint PRIMARY KEY, value text NOT NULL); INSERT INTO {source_schema}.records VALUES (1, 'survives'); CREATE SCHEMA {target_schema}; CREATE TABLE {target_schema}.records (id bigint PRIMARY KEY, value text NOT NULL); INSERT INTO {target_schema}.records VALUES (9, 'rollback');"
        ))
        .unwrap();
    drop(postgres);
    let mut postgres_adapter =
        PostgresAdapter::connect_tls(&postgres_uri, &source_schema, &postgres_ca).unwrap();
    let postgres_snapshot = postgres_adapter.snapshot().unwrap();
    let postgres_plan = postgres_adapter
        .plan(&postgres_snapshot, &target_schema)
        .unwrap();
    let postgres_receipt = postgres_adapter
        .restore(&postgres_snapshot, &postgres_plan)
        .unwrap();
    postgres_adapter
        .verify(&postgres_snapshot, &target_schema)
        .unwrap();
    postgres_adapter.rollback(&postgres_receipt).unwrap();
    let mut postgres = postgres_client(&postgres_uri, &postgres_ca);
    let value: String = postgres
        .query_one(&format!("SELECT value FROM {target_schema}.records"), &[])
        .unwrap()
        .get(0);
    assert_eq!(value, "rollback");
    postgres
        .batch_execute(&format!(
            "DROP SCHEMA {source_schema} CASCADE; DROP SCHEMA {target_schema} CASCADE; DROP SCHEMA IF EXISTS {target_schema}_anasemble_failed CASCADE;"
        ))
        .unwrap();

    let source_stream = format!("source-{suffix}");
    let target_stream = format!("target-{suffix}");
    let redis_client = redis::Client::open(redis_url.clone()).unwrap();
    let mut redis = redis_client
        .get_connection_with_timeout(Duration::from_secs(10))
        .unwrap();
    redis::cmd("XADD")
        .arg(&source_stream)
        .arg("1-0")
        .arg("value")
        .arg("survives")
        .query::<String>(&mut redis)
        .unwrap();
    redis::cmd("XADD")
        .arg(&target_stream)
        .arg("1-0")
        .arg("value")
        .arg("rollback")
        .query::<String>(&mut redis)
        .unwrap();
    drop(redis);
    let mut redis_adapter = RedisStreamAdapter::connect(&redis_url, &source_stream).unwrap();
    let redis_snapshot = redis_adapter.snapshot().unwrap();
    let redis_plan = redis_adapter.plan(&redis_snapshot, &target_stream).unwrap();
    let redis_receipt = redis_adapter.restore(&redis_snapshot, &redis_plan).unwrap();
    redis_adapter
        .verify(&redis_snapshot, &target_stream)
        .unwrap();
    redis_adapter.rollback(&redis_receipt).unwrap();
    let mut redis = redis_client.get_connection().unwrap();
    assert_eq!(redis.xlen::<_, i64>(&target_stream).unwrap(), 1);
    redis::cmd("DEL")
        .arg(&source_stream)
        .arg(&target_stream)
        .arg(format!("{target_stream}:anasemble:failed"))
        .query::<i64>(&mut redis)
        .unwrap();

    let source_prefix = format!("source-{suffix}/");
    let target_prefix = format!("target-{suffix}/");
    let mut s3 = S3Adapter::connect_with_token(
        &s3_endpoint,
        &s3_region,
        &s3_bucket,
        &s3_access_key,
        &s3_secret_key,
        Some(&s3_session_token),
        &source_prefix,
    )
    .unwrap();
    s3.put_object(&format!("{source_prefix}record.bin"), b"survives")
        .unwrap();
    s3.put_object(&format!("{target_prefix}record.bin"), b"rollback")
        .unwrap();
    let s3_snapshot = s3.snapshot().unwrap();
    let s3_plan = s3.plan(&s3_snapshot, &target_prefix).unwrap();
    let s3_receipt = s3.restore(&s3_snapshot, &s3_plan).unwrap();
    s3.verify(&s3_snapshot, &target_prefix).unwrap();
    s3.rollback(&s3_receipt).unwrap();
    assert_eq!(
        s3.get_object(&format!("{target_prefix}record.bin"))
            .unwrap(),
        b"rollback"
    );
}

#[test]
#[ignore = "requires a tagged ephemeral EKS cluster with VPC CNI policy enforcement"]
fn aws_eks_activation_switches_rolls_back_and_denies_egress() {
    let context = required("ANASEMBLE_AWS_KUBE_CONTEXT");
    let image = required("ANASEMBLE_AWS_KUBE_IMAGE");
    let suffix = required("ANASEMBLE_AWS_RUN_SUFFIX");
    let namespace = format!("anasemble-{suffix}");
    let cleanup = NamespaceCleanup {
        context: context.clone(),
        namespace: namespace.clone(),
    };
    kubectl(&context, &["create", "namespace", &namespace]);
    kubectl(
        &context,
        &[
            "create",
            "secret",
            "generic",
            "service-token",
            "--namespace",
            &namespace,
            "--from-literal=token=ephemeral-evaluation-secret",
        ],
    );

    let key = SigningKey::from_bytes(&[0x53; 32]);
    let policy = ApprovalPolicy {
        operator_keys: BTreeMap::from([("operator-a".into(), key.verifying_key().to_bytes())]),
        not_before: "2026-08-11T00:00:00Z".into(),
        not_after: "2026-08-12T00:00:00Z".into(),
    };
    let orchestrator = KubernetesOrchestrator::new(&context, policy).unwrap();
    let make_spec = |seed: &str| {
        let plan = activation_plan(seed);
        let artifact = registry_receipt(&plan, &image);
        let spec = KubernetesDeploymentSpec {
            version: "kubernetes-deployment-v1".into(),
            namespace: namespace.clone(),
            service: "turnstile".into(),
            artifact,
            command: vec!["/bin/sleep".into(), "300".into()],
            isolation: IsolationPolicy {
                cpu_millis: 250,
                memory_bytes: 64 * 1024 * 1024,
                pids: 16,
                wall_time_ms: 5_000,
                output_bytes: 65_536,
                writable_tmpfs_bytes: 4 * 1024 * 1024,
                linux_capabilities: Vec::new(),
                network_egress_allowlist: Vec::new(),
            },
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
                attempts: 60,
                interval_ms: 1_000,
            },
            service_port: 8080,
            container_port: 8080,
        };
        (plan, spec)
    };

    let (old_plan, old_spec) = make_spec("eks-old");
    orchestrator
        .activate(&old_spec, &approval(&key, &old_plan, &old_spec.artifact))
        .unwrap();
    orchestrator.commit(&namespace, "turnstile").unwrap();
    let (new_plan, new_spec) = make_spec("eks-new");
    orchestrator
        .activate(&new_spec, &approval(&key, &new_plan, &new_spec.artifact))
        .unwrap();

    let deployment = format!("turnstile-stage-{}", &new_plan.plan_sha256[..12]);
    let denied = Command::new("kubectl")
        .args([
            "--context",
            &context,
            "exec",
            &format!("deployment/{deployment}"),
            "--namespace",
            &namespace,
            "--",
            "/usr/bin/timeout",
            "3",
            "/bin/bash",
            "-c",
            "</dev/tcp/1.1.1.1/443",
        ])
        .output()
        .unwrap();
    assert!(
        !denied.status.success(),
        "zero-egress policy was not enforced"
    );
    orchestrator.rollback(&namespace, "turnstile").unwrap();
    let selected = kubectl(
        &context,
        &[
            "get",
            "service",
            "turnstile",
            "--namespace",
            &namespace,
            "-o",
            "jsonpath={.spec.selector.anasemble\\.plan}",
        ],
    );
    assert_eq!(selected, old_plan.plan_sha256[..63]);
    drop(cleanup);
}

struct NamespaceCleanup {
    context: String,
    namespace: String,
}

impl Drop for NamespaceCleanup {
    fn drop(&mut self) {
        let _ = Command::new("kubectl")
            .args([
                "--context",
                &self.context,
                "delete",
                "namespace",
                &self.namespace,
                "--wait=true",
                "--timeout=60s",
            ])
            .output();
    }
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

fn registry_receipt(plan: &ActivationPlan, image: &str) -> RegistryReceipt {
    let (repository, manifest_sha256) = image.rsplit_once("@sha256:").unwrap();
    let mut receipt = RegistryReceipt {
        repository: repository.into(),
        immutable_image: image.into(),
        manifest_sha256: manifest_sha256.into(),
        activation_plan_sha256: plan.plan_sha256.clone(),
        candidate_sha256: plan.candidate_sha256.clone(),
        binding_sha256: String::new(),
    };
    receipt.binding_sha256 = anasemble::canonical::digest(&receipt).unwrap();
    receipt
}

fn approval(
    key: &SigningKey,
    plan: &ActivationPlan,
    artifact: &RegistryReceipt,
) -> OperatorApproval {
    let mut approval = OperatorApproval {
        version: "operator-approval-v1".into(),
        plan_sha256: plan.plan_sha256.clone(),
        artifact_sha256: artifact.binding_sha256.clone(),
        operator_key_id: "operator-a".into(),
        approved_at: "2026-08-11T12:00:00Z".into(),
        signature: String::new(),
    };
    approval.signature = hex::encode(key.sign(&approval_payload(&approval).unwrap()).to_bytes());
    approval
}

fn kubectl(context: &str, args: &[&str]) -> String {
    let output = Command::new("kubectl")
        .args(["--context", context])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "kubectl failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn postgres_client(uri: &str, ca_pem: &[u8]) -> Client {
    let mut roots = RootCertStore::empty();
    let certificates = CertificateDer::pem_slice_iter(ca_pem)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    for certificate in certificates {
        roots.add(certificate).unwrap();
    }
    let config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
    Client::connect(uri, MakeRustlsConnect::new(config)).unwrap()
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} is required"))
}
