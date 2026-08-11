//! Bounded public reference workflow composing recovery, state and activation.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::activation::{
    ActivationReceipt, ApprovalPolicy, ArtifactPackageRequest, DockerRegistry, HealthProbe,
    IsolationPolicy, KubernetesDeploymentSpec, KubernetesOrchestrator, KubernetesSecretReference,
    OperatorApproval, RegistryReceipt, approval_payload, import_image_into_kind_node,
};
use crate::canonical::{digest, encode};
use crate::fragments::{read_signing_key, sign_detached_with_key_file};
use crate::model::Error;
use crate::protocol::{RecoveryResult, run};
use crate::service::ServiceManifest;
use crate::stateful::{
    ActivationPlan, PostgresAdapter, RedisStreamAdapter, RestoreReceipt, S3Adapter, StateSnapshot,
    TransactionalStateAdapter, bind_activation_plan,
};

const MAX_CONFIG_BYTES: u64 = 65_536;
const MAX_BUNDLE_BYTES: u64 = 192 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceRecoveryConfig {
    pub version: String,
    pub workspace: PathBuf,
    pub service_manifest: PathBuf,
    pub postgres: PostgresReference,
    pub s3: S3Reference,
    pub redis: RedisReference,
    pub artifact: ArtifactReference,
    pub kubernetes: KubernetesReference,
    pub operator: OperatorReference,
    pub isolation: IsolationPolicy,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PostgresReference {
    pub connection_file: PathBuf,
    pub source_schema: String,
    pub target_schema: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct S3Reference {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key_file: PathBuf,
    pub secret_key_file: PathBuf,
    pub source_prefix: String,
    pub target_prefix: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RedisReference {
    pub url_file: PathBuf,
    pub source_stream: String,
    pub target_stream: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactReference {
    pub base_image: String,
    pub local_image: String,
    pub registry_endpoint: String,
    pub repository: String,
    pub tag: String,
    #[serde(default)]
    pub kind_import: Option<KindImport>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KindImport {
    pub node: String,
    pub platform: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KubernetesReference {
    pub context: String,
    pub namespace: String,
    pub service: String,
    pub command: Vec<String>,
    pub health_command: Vec<String>,
    pub health_attempts: u32,
    pub health_interval_ms: u64,
    pub service_port: u16,
    pub container_port: u16,
    pub secrets: Vec<KubernetesSecretReference>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorReference {
    pub signing_key: PathBuf,
    pub not_before: String,
    pub not_after: String,
    pub approved_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceStateBundle {
    pub version: String,
    pub service_manifest_sha256: String,
    pub postgres: StateSnapshot,
    pub s3: StateSnapshot,
    pub redis: StateSnapshot,
    pub bundle_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceRecoveryReceipt {
    pub version: String,
    pub config_sha256: String,
    pub activation_plan: ActivationPlan,
    pub postgres: RestoreReceipt,
    pub s3: RestoreReceipt,
    pub redis: RestoreReceipt,
    pub artifact: RegistryReceipt,
    pub activation: ActivationReceipt,
}

pub fn read_config(path: &Path) -> Result<ReferenceRecoveryConfig, Error> {
    let config: ReferenceRecoveryConfig = serde_json::from_slice(&read_regular(
        path,
        MAX_CONFIG_BYTES,
        false,
        "reference configuration",
    )?)?;
    validate_config(&config)?;
    Ok(config)
}

pub fn read_bundle(path: &Path) -> Result<ReferenceStateBundle, Error> {
    let bundle: ReferenceStateBundle = serde_json::from_slice(&read_regular(
        path,
        MAX_BUNDLE_BYTES,
        true,
        "reference state bundle",
    )?)?;
    validate_bundle(&bundle)?;
    Ok(bundle)
}

pub fn read_receipt(path: &Path) -> Result<ReferenceRecoveryReceipt, Error> {
    let receipt: ReferenceRecoveryReceipt = serde_json::from_slice(&read_regular(
        path,
        MAX_CONFIG_BYTES,
        true,
        "reference recovery receipt",
    )?)?;
    if receipt.version != "reference-recovery-receipt-v1" {
        return Err(invalid("reference recovery receipt version is unsupported"));
    }
    Ok(receipt)
}

pub fn prepare(config: &ReferenceRecoveryConfig) -> Result<ReferenceStateBundle, Error> {
    validate_config(config)?;
    let service = read_service(config)?;
    let mut postgres = postgres_adapter(config)?;
    let mut s3 = s3_adapter(config)?;
    let mut redis = redis_adapter(config)?;
    let mut bundle = ReferenceStateBundle {
        version: "reference-state-bundle-v1".into(),
        service_manifest_sha256: digest(&service)?,
        postgres: postgres.snapshot()?,
        s3: s3.snapshot()?,
        redis: redis.snapshot()?,
        bundle_sha256: String::new(),
    };
    bundle.bundle_sha256 = digest(&bundle)?;
    Ok(bundle)
}

pub fn recover_and_activate(
    config: &ReferenceRecoveryConfig,
    bundle: &ReferenceStateBundle,
) -> Result<ReferenceRecoveryReceipt, Error> {
    validate_config(config)?;
    validate_bundle(bundle)?;
    let service = read_service(config)?;
    let service_sha256 = digest(&service)?;
    if service_sha256 != bundle.service_manifest_sha256 {
        return Err(invalid(
            "service manifest differs from the prepared state bundle",
        ));
    }
    let recovery = run(&config.workspace);
    let candidate = match &recovery {
        RecoveryResult::Certified { candidate, .. } => encode(candidate.as_ref())?,
        RecoveryResult::Refused { .. } => {
            return Err(invalid("reference recovery did not certify a candidate"));
        }
    };
    let service_bytes = encode(&service)?;
    let registry = DockerRegistry::connect(&config.artifact.registry_endpoint)?;
    let key = read_signing_key(&config.operator.signing_key)?;
    let public_key: [u8; 32] = hex::decode(&key.public_key_hex)
        .map_err(|_| invalid("operator public key is not hex"))?
        .try_into()
        .map_err(|_| invalid("operator public key length is invalid"))?;
    let orchestrator = KubernetesOrchestrator::new(
        &config.kubernetes.context,
        ApprovalPolicy {
            operator_keys: BTreeMap::from([(key.key_id.clone(), public_key)]),
            not_before: config.operator.not_before.clone(),
            not_after: config.operator.not_after.clone(),
        },
    )?;
    let mut postgres = postgres_adapter(config)?;
    let mut s3 = s3_adapter(config)?;
    let mut redis = redis_adapter(config)?;
    let postgres_plan = postgres.plan(&bundle.postgres, &config.postgres.target_schema)?;
    let s3_plan = s3.plan(&bundle.s3, &config.s3.target_prefix)?;
    let redis_plan = redis.plan(&bundle.redis, &config.redis.target_stream)?;

    let postgres_receipt = postgres.restore(&bundle.postgres, &postgres_plan)?;
    let s3_receipt = match s3.restore(&bundle.s3, &s3_plan) {
        Ok(receipt) => receipt,
        Err(error) => {
            return Err(rollback_failure(
                error,
                &[postgres.rollback(&postgres_receipt)],
            ));
        }
    };
    let redis_receipt = match redis.restore(&bundle.redis, &redis_plan) {
        Ok(receipt) => receipt,
        Err(error) => {
            return Err(rollback_failure(
                error,
                &[
                    s3.rollback(&s3_receipt),
                    postgres.rollback(&postgres_receipt),
                ],
            ));
        }
    };

    let activation_plan = match bind_activation_plan(
        &recovery,
        &service_sha256,
        &[
            (&bundle.postgres, &postgres_plan),
            (&bundle.s3, &s3_plan),
            (&bundle.redis, &redis_plan),
        ],
    ) {
        Ok(plan) => plan,
        Err(error) => {
            return Err(rollback_failure(
                error,
                &[
                    redis.rollback(&redis_receipt),
                    s3.rollback(&s3_receipt),
                    postgres.rollback(&postgres_receipt),
                ],
            ));
        }
    };
    let artifact = match registry.package_and_publish(&ArtifactPackageRequest {
        base_image: config.artifact.base_image.clone(),
        local_image: config.artifact.local_image.clone(),
        repository: config.artifact.repository.clone(),
        tag: config.artifact.tag.clone(),
        activation_plan: activation_plan.clone(),
        candidate,
        service_manifest: service_bytes,
    }) {
        Ok(receipt) => receipt,
        Err(error) => {
            return Err(rollback_failure(
                error,
                &[
                    redis.rollback(&redis_receipt),
                    s3.rollback(&s3_receipt),
                    postgres.rollback(&postgres_receipt),
                ],
            ));
        }
    };
    if let Some(import) = &config.artifact.kind_import
        && let Err(error) =
            import_image_into_kind_node(&artifact.immutable_image, &import.node, &import.platform)
    {
        return Err(rollback_failure(
            error,
            &[
                redis.rollback(&redis_receipt),
                s3.rollback(&s3_receipt),
                postgres.rollback(&postgres_receipt),
            ],
        ));
    }
    let spec = kubernetes_spec(config, artifact.clone());
    let mut approval = OperatorApproval {
        version: "operator-approval-v1".into(),
        plan_sha256: activation_plan.plan_sha256.clone(),
        artifact_sha256: artifact.binding_sha256.clone(),
        operator_key_id: key.key_id.clone(),
        approved_at: config.operator.approved_at.clone(),
        signature: String::new(),
    };
    approval.signature = match approval_payload(&approval)
        .and_then(|payload| sign_detached_with_key_file(&payload, &key))
    {
        Ok(signature) => signature,
        Err(error) => {
            return Err(rollback_failure(
                error,
                &[
                    redis.rollback(&redis_receipt),
                    s3.rollback(&s3_receipt),
                    postgres.rollback(&postgres_receipt),
                ],
            ));
        }
    };
    let activation = match orchestrator.activate(&spec, &approval) {
        Ok(receipt) => receipt,
        Err(error) => {
            return Err(rollback_failure(
                error,
                &[
                    redis.rollback(&redis_receipt),
                    s3.rollback(&s3_receipt),
                    postgres.rollback(&postgres_receipt),
                ],
            ));
        }
    };
    Ok(ReferenceRecoveryReceipt {
        version: "reference-recovery-receipt-v1".into(),
        config_sha256: digest(config)?,
        activation_plan,
        postgres: postgres_receipt,
        s3: s3_receipt,
        redis: redis_receipt,
        artifact,
        activation,
    })
}

pub fn rollback_recovery(
    config: &ReferenceRecoveryConfig,
    receipt: &ReferenceRecoveryReceipt,
) -> Result<(), Error> {
    validate_config(config)?;
    if receipt.version != "reference-recovery-receipt-v1"
        || receipt.config_sha256 != digest(config)?
    {
        return Err(invalid("recovery receipt does not bind this configuration"));
    }
    let orchestrator = KubernetesOrchestrator::new(
        &config.kubernetes.context,
        ApprovalPolicy {
            operator_keys: BTreeMap::new(),
            not_before: config.operator.not_before.clone(),
            not_after: config.operator.not_after.clone(),
        },
    )?;
    orchestrator.rollback(&config.kubernetes.namespace, &config.kubernetes.service)?;
    let mut postgres = postgres_adapter(config)?;
    let mut s3 = s3_adapter(config)?;
    let mut redis = redis_adapter(config)?;
    let outcomes = [
        redis.rollback(&receipt.redis),
        s3.rollback(&receipt.s3),
        postgres.rollback(&receipt.postgres),
    ];
    let errors: Vec<String> = outcomes
        .into_iter()
        .filter_map(Result::err)
        .map(|error| error.to_string())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(invalid(&format!(
            "one or more state rollbacks failed: {}",
            errors.join("; ")
        )))
    }
}

fn kubernetes_spec(
    config: &ReferenceRecoveryConfig,
    artifact: RegistryReceipt,
) -> KubernetesDeploymentSpec {
    KubernetesDeploymentSpec {
        version: "kubernetes-deployment-v1".into(),
        namespace: config.kubernetes.namespace.clone(),
        service: config.kubernetes.service.clone(),
        artifact,
        command: config.kubernetes.command.clone(),
        isolation: config.isolation.clone(),
        secrets: config.kubernetes.secrets.clone(),
        health: HealthProbe {
            command: config.kubernetes.health_command.clone(),
            attempts: config.kubernetes.health_attempts,
            interval_ms: config.kubernetes.health_interval_ms,
        },
        service_port: config.kubernetes.service_port,
        container_port: config.kubernetes.container_port,
    }
}

fn validate_config(config: &ReferenceRecoveryConfig) -> Result<(), Error> {
    if config.version != "reference-recovery-config-v1" {
        return Err(invalid(
            "reference recovery configuration version is unsupported",
        ));
    }
    config.isolation.validate()?;
    if config.kubernetes.health_attempts == 0
        || config.kubernetes.health_attempts > 300
        || config.kubernetes.health_interval_ms == 0
        || config.kubernetes.health_interval_ms > 60_000
    {
        return Err(invalid(
            "reference health policy is outside supported bounds",
        ));
    }
    Ok(())
}

fn validate_bundle(bundle: &ReferenceStateBundle) -> Result<(), Error> {
    if bundle.version != "reference-state-bundle-v1" || bundle.bundle_sha256.is_empty() {
        return Err(invalid(
            "reference state bundle version or digest is invalid",
        ));
    }
    let mut unsigned = bundle.clone();
    let expected = std::mem::take(&mut unsigned.bundle_sha256);
    if digest(&unsigned)? != expected {
        return Err(invalid("reference state bundle digest is invalid"));
    }
    Ok(())
}

fn read_service(config: &ReferenceRecoveryConfig) -> Result<ServiceManifest, Error> {
    let service: ServiceManifest = serde_json::from_slice(&read_regular(
        &config.service_manifest,
        MAX_CONFIG_BYTES,
        false,
        "service manifest",
    )?)?;
    service.validate()?;
    Ok(service)
}

fn postgres_adapter(config: &ReferenceRecoveryConfig) -> Result<PostgresAdapter, Error> {
    PostgresAdapter::connect(
        &read_secret_string(&config.postgres.connection_file)?,
        &config.postgres.source_schema,
    )
}

fn s3_adapter(config: &ReferenceRecoveryConfig) -> Result<S3Adapter, Error> {
    S3Adapter::connect(
        &config.s3.endpoint,
        &config.s3.region,
        &config.s3.bucket,
        &read_secret_string(&config.s3.access_key_file)?,
        &read_secret_string(&config.s3.secret_key_file)?,
        &config.s3.source_prefix,
    )
}

fn redis_adapter(config: &ReferenceRecoveryConfig) -> Result<RedisStreamAdapter, Error> {
    RedisStreamAdapter::connect(
        &read_secret_string(&config.redis.url_file)?,
        &config.redis.source_stream,
    )
}

fn read_secret_string(path: &Path) -> Result<String, Error> {
    let bytes = read_regular(path, 4096, true, "credential reference")?;
    let value =
        String::from_utf8(bytes).map_err(|_| invalid("credential reference is not UTF-8"))?;
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(invalid(
            "credential reference is empty or contains control bytes",
        ));
    }
    Ok(value.into())
}

fn read_regular(path: &Path, maximum: u64, private: bool, label: &str) -> Result<Vec<u8>, Error> {
    let metadata = fs::symlink_metadata(path).map_err(Error::Io)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > maximum
        || private && metadata.permissions().mode() & 0o077 != 0
    {
        return Err(invalid(&format!(
            "{label} is not a valid bounded regular file"
        )));
    }
    fs::read(path).map_err(Error::Io)
}

fn rollback_failure(original: Error, outcomes: &[Result<(), Error>]) -> Error {
    let failures: Vec<String> = outcomes
        .iter()
        .filter_map(|outcome| outcome.as_ref().err())
        .map(ToString::to_string)
        .collect();
    if failures.is_empty() {
        invalid(&format!("{original}; completed reverse-order rollback"))
    } else {
        invalid(&format!(
            "{original}; rollback failures require operator action: {}",
            failures.join("; ")
        ))
    }
}

fn invalid(message: &str) -> Error {
    Error::InvalidEvidence(message.into())
}
