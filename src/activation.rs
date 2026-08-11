//! Docker-backed isolation and activation for certified immutable artifacts.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::str::FromStr;
use std::thread;
use std::time::{Duration, Instant};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::canonical::encode;
use crate::model::Error;
use crate::stateful::{ActivationPlan, validate_activation_plan};

const MAX_DOCKER_OUTPUT: usize = 1_048_576;
const MAX_ARGUMENTS: usize = 128;
const MAX_ARGUMENT_BYTES: usize = 16_384;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IsolationPolicy {
    pub cpu_millis: u32,
    pub memory_bytes: u64,
    pub pids: u32,
    pub wall_time_ms: u64,
    pub output_bytes: usize,
    pub writable_tmpfs_bytes: u64,
    pub linux_capabilities: Vec<String>,
    pub network_egress_allowlist: Vec<String>,
}

impl IsolationPolicy {
    pub fn validate(&self) -> Result<(), Error> {
        if !(10..=4_000).contains(&self.cpu_millis)
            || !(16 * 1024 * 1024..=2 * 1024 * 1024 * 1024).contains(&self.memory_bytes)
            || !(1..=512).contains(&self.pids)
            || !(10..=300_000).contains(&self.wall_time_ms)
            || !(1..=MAX_DOCKER_OUTPUT).contains(&self.output_bytes)
            || !(1024 * 1024..=256 * 1024 * 1024).contains(&self.writable_tmpfs_bytes)
        {
            return Err(invalid(
                "isolation resource policy is outside supported bounds",
            ));
        }
        if !self.network_egress_allowlist.is_empty() {
            return Err(invalid(
                "the Docker profile supports only an empty network egress allowlist",
            ));
        }
        if self.linux_capabilities.len() > 16 {
            return Err(invalid("capability allowlist exceeds sixteen entries"));
        }
        let supported = BTreeSet::from([
            "CHOWN",
            "DAC_OVERRIDE",
            "FOWNER",
            "KILL",
            "NET_BIND_SERVICE",
            "SETGID",
            "SETUID",
        ]);
        if self
            .linux_capabilities
            .iter()
            .any(|capability| !supported.contains(capability.as_str()))
        {
            return Err(invalid(
                "capability allowlist contains an unsupported capability",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SecretReference {
    pub id: String,
    pub source_file: PathBuf,
    pub mount_path: String,
}

impl SecretReference {
    fn validate(&self) -> Result<(), Error> {
        validate_label(&self.id)?;
        if !self.mount_path.starts_with("/run/secrets/")
            || self.mount_path.len() > 256
            || self.mount_path.contains("..")
            || self.mount_path.chars().any(char::is_control)
        {
            return Err(invalid("secret mount path must be below /run/secrets"));
        }
        let metadata = fs::symlink_metadata(&self.source_file).map_err(Error::Io)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o077 != 0
            || metadata.len() == 0
            || metadata.len() > 65_536
        {
            return Err(invalid(
                "secret source must be a non-empty owner-only regular file up to 64 KiB",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SandboxRequest {
    pub image: String,
    pub command: Vec<String>,
    pub policy: IsolationPolicy,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SandboxReceipt {
    pub image: String,
    pub exit_code: i64,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
    pub policy_sha256: String,
}

pub struct DockerSandbox;

impl DockerSandbox {
    pub fn run(request: &SandboxRequest) -> Result<SandboxReceipt, Error> {
        request.policy.validate()?;
        validate_immutable_image(&request.image)?;
        validate_arguments(&request.command)?;
        let name = unique_name("anasemble-sandbox");
        let mut args = vec![
            "create".into(),
            "--name".into(),
            name.clone(),
            "--network".into(),
            "none".into(),
            "--read-only".into(),
            "--cap-drop".into(),
            "ALL".into(),
            "--security-opt".into(),
            "no-new-privileges".into(),
            "--pids-limit".into(),
            request.policy.pids.to_string(),
            "--memory".into(),
            request.policy.memory_bytes.to_string(),
            "--memory-swap".into(),
            request.policy.memory_bytes.to_string(),
            "--cpus".into(),
            format!("{:.3}", f64::from(request.policy.cpu_millis) / 1_000.0),
            "--tmpfs".into(),
            format!(
                "/tmp:rw,noexec,nosuid,nodev,size={}",
                request.policy.writable_tmpfs_bytes
            ),
            "--log-opt".into(),
            "max-size=1m".into(),
            "--log-opt".into(),
            "max-file=1".into(),
        ];
        for capability in &request.policy.linux_capabilities {
            args.extend(["--cap-add".into(), capability.clone()]);
        }
        args.push(request.image.clone());
        args.extend(request.command.clone());
        docker(&args)?;
        let guard = ContainerGuard::new(name.clone());
        docker(&["start".into(), name.clone()])?;
        let deadline = Instant::now() + Duration::from_millis(request.policy.wall_time_ms);
        let (exit_code, timed_out) = loop {
            let running = inspect_bool(&name, "{{.State.Running}}")?;
            if !running {
                break (inspect_i64(&name, "{{.State.ExitCode}}")?, false);
            }
            if Instant::now() >= deadline {
                docker(&["kill".into(), name.clone()])?;
                break (inspect_i64(&name, "{{.State.ExitCode}}")?, true);
            }
            thread::sleep(Duration::from_millis(20));
        };
        let logs = docker_raw(&["logs".into(), name.clone()])?;
        if logs.stdout.len().saturating_add(logs.stderr.len()) > request.policy.output_bytes {
            return Err(invalid("sandbox output exceeded the configured byte bound"));
        }
        drop(guard);
        Ok(SandboxReceipt {
            image: request.image.clone(),
            exit_code,
            stdout: logs.stdout,
            stderr: logs.stderr,
            timed_out,
            policy_sha256: crate::canonical::digest(&request.policy)?,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OperatorApproval {
    pub version: String,
    pub plan_sha256: String,
    pub artifact_sha256: String,
    pub operator_key_id: String,
    pub approved_at: String,
    pub signature: String,
}

#[derive(Serialize)]
struct ApprovalPayload<'a> {
    version: &'a str,
    plan_sha256: &'a str,
    artifact_sha256: &'a str,
    operator_key_id: &'a str,
    approved_at: &'a str,
}

pub fn approval_payload(approval: &OperatorApproval) -> Result<Vec<u8>, Error> {
    encode(&ApprovalPayload {
        version: &approval.version,
        plan_sha256: &approval.plan_sha256,
        artifact_sha256: &approval.artifact_sha256,
        operator_key_id: &approval.operator_key_id,
        approved_at: &approval.approved_at,
    })
}

#[derive(Clone, Debug)]
pub struct ApprovalPolicy {
    pub operator_keys: BTreeMap<String, [u8; 32]>,
    pub not_before: String,
    pub not_after: String,
}

impl ApprovalPolicy {
    pub fn verify(
        &self,
        approval: &OperatorApproval,
        plan_sha256: &str,
        artifact_sha256: &str,
    ) -> Result<(), Error> {
        if self.operator_keys.is_empty() || self.operator_keys.len() > 64 {
            return Err(invalid(
                "operator trust policy must contain one to sixty-four keys",
            ));
        }
        for key_id in self.operator_keys.keys() {
            validate_label(key_id)?;
        }
        if approval.version != "operator-approval-v1"
            || approval.plan_sha256 != plan_sha256
            || approval.artifact_sha256 != artifact_sha256
        {
            return Err(invalid(
                "operator approval does not bind the activation plan",
            ));
        }
        validate_digest(plan_sha256)?;
        validate_digest(artifact_sha256)?;
        let approved_at = jiff::Timestamp::from_str(&approval.approved_at).map_err(backend)?;
        let not_before = jiff::Timestamp::from_str(&self.not_before).map_err(backend)?;
        let not_after = jiff::Timestamp::from_str(&self.not_after).map_err(backend)?;
        if not_before > approved_at || approved_at > not_after || not_before > not_after {
            return Err(invalid(
                "operator approval is outside its trusted time window",
            ));
        }
        let key = self
            .operator_keys
            .get(&approval.operator_key_id)
            .ok_or_else(|| invalid("operator approval key is not trusted"))?;
        let verifying_key = VerifyingKey::from_bytes(key).map_err(backend)?;
        let signature_bytes: [u8; 64] = hex::decode(&approval.signature)
            .map_err(backend)?
            .try_into()
            .map_err(|_| invalid("operator approval signature length is invalid"))?;
        verifying_key
            .verify(
                &approval_payload(approval)?,
                &Signature::from_bytes(&signature_bytes),
            )
            .map_err(|_| invalid("operator approval signature is invalid"))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RegistryReceipt {
    pub repository: String,
    pub immutable_image: String,
    pub manifest_sha256: String,
    pub activation_plan_sha256: String,
    pub candidate_sha256: String,
    pub binding_sha256: String,
}

#[derive(Clone, Debug)]
pub struct ArtifactPackageRequest {
    pub base_image: String,
    pub local_image: String,
    pub repository: String,
    pub tag: String,
    pub activation_plan: ActivationPlan,
    pub candidate: Vec<u8>,
    pub service_manifest: Vec<u8>,
}

pub struct DockerRegistry {
    endpoint: String,
}

impl DockerRegistry {
    pub fn connect(endpoint: &str) -> Result<Self, Error> {
        if endpoint.is_empty()
            || endpoint.len() > 253
            || endpoint.contains('/')
            || endpoint.chars().any(char::is_control)
        {
            return Err(invalid("OCI registry endpoint is invalid"));
        }
        Ok(Self {
            endpoint: endpoint.into(),
        })
    }

    pub fn publish(
        &self,
        local_image: &str,
        repository: &str,
        tag: &str,
        activation_plan: &ActivationPlan,
    ) -> Result<RegistryReceipt, Error> {
        validate_activation_plan(activation_plan)?;
        validate_label(repository)?;
        validate_label(tag)?;
        let image_plan = docker_text(&[
            "image".into(),
            "inspect".into(),
            "--format".into(),
            "{{index .Config.Labels \"anasemble.plan\"}}".into(),
            local_image.into(),
        ])?;
        let image_candidate = docker_text(&[
            "image".into(),
            "inspect".into(),
            "--format".into(),
            "{{index .Config.Labels \"anasemble.candidate\"}}".into(),
            local_image.into(),
        ])?;
        if image_plan.trim() != activation_plan.plan_sha256
            || image_candidate.trim() != activation_plan.candidate_sha256
        {
            return Err(invalid(
                "local artifact labels do not bind the activation plan and candidate",
            ));
        }
        let target = format!("{}/{repository}:{tag}", self.endpoint);
        docker(&["tag".into(), local_image.into(), target.clone()])?;
        let mut pushed = false;
        let mut last_error = None;
        for attempt in 0..20 {
            match docker(&["push".into(), target.clone()]) {
                Ok(()) => {
                    pushed = true;
                    break;
                }
                Err(error) => last_error = Some(error.to_string()),
            }
            if attempt < 19 {
                thread::sleep(Duration::from_millis(100));
            }
        }
        if !pushed {
            return Err(invalid(&format!(
                "OCI registry push exhausted its bounded retry budget: {}",
                last_error.as_deref().unwrap_or("unknown Docker failure")
            )));
        }
        let output = docker_text(&[
            "image".into(),
            "inspect".into(),
            "--format".into(),
            "{{json .RepoDigests}}".into(),
            target,
        ])?;
        let digests: Vec<String> = serde_json::from_str(output.trim()).map_err(backend)?;
        let prefix = format!("{}/{repository}@sha256:", self.endpoint);
        let immutable_image = digests
            .into_iter()
            .find(|digest| digest.starts_with(&prefix))
            .ok_or_else(|| invalid("registry push did not produce an immutable digest"))?;
        let manifest_sha256 = immutable_image
            .rsplit_once("sha256:")
            .map(|(_, digest)| digest.to_owned())
            .ok_or_else(|| invalid("registry digest is malformed"))?;
        validate_digest(&manifest_sha256)?;
        let mut receipt = RegistryReceipt {
            repository: format!("{}/{}", self.endpoint, repository),
            immutable_image,
            manifest_sha256,
            activation_plan_sha256: activation_plan.plan_sha256.clone(),
            candidate_sha256: activation_plan.candidate_sha256.clone(),
            binding_sha256: String::new(),
        };
        receipt.binding_sha256 = crate::canonical::digest(&receipt)?;
        Ok(receipt)
    }

    pub fn package_and_publish(
        &self,
        request: &ArtifactPackageRequest,
    ) -> Result<RegistryReceipt, Error> {
        validate_immutable_image(&request.base_image)?;
        validate_local_image(&request.local_image)?;
        if request.candidate.is_empty()
            || request.candidate.len() > 16 * 1024 * 1024
            || request.service_manifest.is_empty()
            || request.service_manifest.len() > 65_536
        {
            return Err(invalid(
                "artifact payload is empty or exceeds its byte bound",
            ));
        }
        if image_exists(&request.local_image)? {
            return Err(invalid("local artifact image already exists"));
        }
        let root = std::env::temp_dir().join(unique_name("anasemble-package"));
        fs::create_dir(&root).map_err(Error::Io)?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).map_err(Error::Io)?;
        let mut guard = PackageGuard::new(root.clone());
        write_public_payload(&root.join("candidate.json"), &request.candidate)?;
        write_public_payload(&root.join("service.json"), &request.service_manifest)?;
        let container = unique_name("anasemble-package");
        docker(&[
            "create".into(),
            "--name".into(),
            container.clone(),
            request.base_image.clone(),
            "/bin/true".into(),
        ])?;
        guard.container = Some(container.clone());
        docker(&[
            "cp".into(),
            root.join("candidate.json").display().to_string(),
            format!("{container}:/candidate.json"),
        ])?;
        docker(&[
            "cp".into(),
            root.join("service.json").display().to_string(),
            format!("{container}:/service.json"),
        ])?;
        docker(&[
            "commit".into(),
            "--change".into(),
            format!(
                "LABEL anasemble.plan={}",
                request.activation_plan.plan_sha256
            ),
            "--change".into(),
            format!(
                "LABEL anasemble.candidate={}",
                request.activation_plan.candidate_sha256
            ),
            container,
            request.local_image.clone(),
        ])?;
        self.publish(
            &request.local_image,
            &request.repository,
            &request.tag,
            &request.activation_plan,
        )
    }
}

pub fn import_image_into_kind_node(image: &str, node: &str, platform: &str) -> Result<(), Error> {
    validate_immutable_image(image)?;
    validate_label(node)?;
    if !matches!(platform, "linux/arm64" | "linux/amd64") {
        return Err(invalid("kind import platform is unsupported"));
    }
    let mut save = Command::new("docker")
        .args(["save", image])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(Error::Io)?;
    let input = save
        .stdout
        .take()
        .ok_or_else(|| invalid("docker save stdout is unavailable"))?;
    let import = Command::new("docker")
        .args([
            "exec",
            "-i",
            node,
            "ctr",
            "--namespace=k8s.io",
            "images",
            "import",
            "--platform",
            platform,
            "--digests",
            "-",
        ])
        .stdin(Stdio::from(input))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(Error::Io)?;
    let saved = save.wait().map_err(Error::Io)?;
    if !saved.success() || !import.status.success() {
        return Err(invalid(&format!(
            "kind image import failed: {}",
            String::from_utf8_lossy(&import.stderr).trim()
        )));
    }
    let digest = image
        .rsplit_once('@')
        .map(|(_, digest)| digest)
        .ok_or_else(|| invalid("kind import image digest is malformed"))?;
    let listed = Command::new("docker")
        .args([
            "exec",
            node,
            "ctr",
            "--namespace=k8s.io",
            "images",
            "list",
            "--quiet",
        ])
        .output()
        .map_err(Error::Io)?;
    if !listed.status.success()
        || listed.stdout.len().saturating_add(listed.stderr.len()) > MAX_DOCKER_OUTPUT
    {
        return Err(invalid("kind image inventory failed"));
    }
    let inventory = String::from_utf8(listed.stdout)
        .map_err(|_| invalid("kind image inventory is not UTF-8"))?;
    let imported = inventory
        .lines()
        .find(|reference| reference.starts_with("import-") && reference.ends_with(digest))
        .ok_or_else(|| invalid("kind import did not retain the OCI manifest digest"))?;
    let tagged = Command::new("docker")
        .args([
            "exec",
            node,
            "ctr",
            "--namespace=k8s.io",
            "images",
            "tag",
            imported,
            image,
        ])
        .output()
        .map_err(Error::Io)?;
    if !tagged.status.success()
        || tagged.stdout.len().saturating_add(tagged.stderr.len()) > MAX_DOCKER_OUTPUT
    {
        return Err(invalid(&format!(
            "kind image reference binding failed: {}",
            String::from_utf8_lossy(&tagged.stderr).trim()
        )));
    }
    Ok(())
}

fn validate_local_image(image: &str) -> Result<(), Error> {
    if image.is_empty()
        || image.len() > 255
        || image.contains('@')
        || image.chars().any(char::is_control)
    {
        return Err(invalid("local artifact image name is invalid"));
    }
    Ok(())
}

fn image_exists(image: &str) -> Result<bool, Error> {
    let output = Command::new("docker")
        .args(["image", "inspect", image])
        .output()
        .map_err(Error::Io)?;
    if output.stdout.len().saturating_add(output.stderr.len()) > MAX_DOCKER_OUTPUT {
        return Err(invalid("Docker image inspection exceeded 1 MiB"));
    }
    Ok(output.status.success())
}

fn write_public_payload(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(path)
        .map_err(Error::Io)?;
    file.write_all(bytes).map_err(Error::Io)?;
    file.sync_all().map_err(Error::Io)
}

struct PackageGuard {
    root: PathBuf,
    container: Option<String>,
}

impl PackageGuard {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            container: None,
        }
    }
}

impl Drop for PackageGuard {
    fn drop(&mut self) {
        if let Some(container) = &self.container {
            let _ = Command::new("docker")
                .args(["rm", "--force", container])
                .output();
        }
        let _ = fs::remove_file(self.root.join("candidate.json"));
        let _ = fs::remove_file(self.root.join("service.json"));
        let _ = fs::remove_dir(&self.root);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HealthProbe {
    pub command: Vec<String>,
    pub attempts: u32,
    pub interval_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeploymentSpec {
    pub version: String,
    pub service: String,
    pub artifact: RegistryReceipt,
    pub command: Vec<String>,
    pub isolation: IsolationPolicy,
    pub secrets: Vec<SecretReference>,
    pub health: HealthProbe,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationFailurePoint {
    None,
    AfterStageHealthy,
    AfterActiveStopped,
    AfterActiveRenamed,
    AfterStageReady,
    AfterServiceSwitched,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KubernetesSecretReference {
    pub id: String,
    pub secret_name: String,
    pub secret_key: String,
    pub mount_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KubernetesDeploymentSpec {
    pub version: String,
    pub namespace: String,
    pub service: String,
    pub artifact: RegistryReceipt,
    pub command: Vec<String>,
    pub isolation: IsolationPolicy,
    pub secrets: Vec<KubernetesSecretReference>,
    pub health: HealthProbe,
    pub service_port: u16,
    pub container_port: u16,
}

pub struct KubernetesOrchestrator {
    context: String,
    approval_policy: ApprovalPolicy,
}

impl KubernetesOrchestrator {
    pub fn new(context: &str, approval_policy: ApprovalPolicy) -> Result<Self, Error> {
        validate_kube_name(context)?;
        Ok(Self {
            context: context.into(),
            approval_policy,
        })
    }
    pub fn activate(
        &self,
        spec: &KubernetesDeploymentSpec,
        approval: &OperatorApproval,
    ) -> Result<ActivationReceipt, Error> {
        self.activate_with_failure(spec, approval, ActivationFailurePoint::None)
    }
    pub fn activate_with_failure(
        &self,
        spec: &KubernetesDeploymentSpec,
        approval: &OperatorApproval,
        failure: ActivationFailurePoint,
    ) -> Result<ActivationReceipt, Error> {
        validate_kubernetes_deployment(spec)?;
        self.approval_policy.verify(
            approval,
            &spec.artifact.activation_plan_sha256,
            &spec.artifact.binding_sha256,
        )?;
        let plan = &spec.artifact.activation_plan_sha256;
        let lease_holder_identity = format!("activate:{plan}");
        let short = &plan[..12];
        let plan_label = &plan[..63];
        let lease = format!("{}-anasemble-lease", spec.service);
        let deployment = format!("{}-stage-{short}", spec.service);
        let lease_holder = self
            .kube_json_optional(&spec.namespace, "lease", &lease)?
            .and_then(|value| {
                value
                    .pointer("/spec/holderIdentity")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            });
        if let Some(holder) = lease_holder {
            if holder != lease_holder_identity {
                return Err(invalid(
                    "another Kubernetes activation lease owns the service",
                ));
            }
        } else {
            let lease_resource = serde_json::json!({"apiVersion":"coordination.k8s.io/v1","kind":"Lease","metadata":{"name":lease,"namespace":spec.namespace},"spec":{"holderIdentity":lease_holder_identity}});
            if self.kube_create(&lease_resource).is_err() {
                return Err(invalid("Kubernetes activation lease acquisition raced"));
            }
        }
        let current = self.kube_json_optional(&spec.namespace, "service", &spec.service)?;
        let current_plan = current
            .as_ref()
            .and_then(|value| value.pointer("/spec/selector/anasemble.plan"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        if current_plan.as_deref() == Some(plan_label) {
            self.kube_delete(&spec.namespace, "lease", &lease)?;
            return Ok(kube_receipt(
                spec,
                approval,
                current
                    .as_ref()
                    .and_then(|v| v.pointer("/metadata/annotations/anasemble.rollback-plan"))
                    .and_then(serde_json::Value::as_str)
                    .is_some(),
                true,
            ));
        }
        self.apply_network_policy(spec)?;
        self.apply_deployment(spec, &deployment)?;
        let timeout_seconds =
            ((spec.health.attempts as u64).saturating_mul(spec.health.interval_ms) / 1_000)
                .clamp(1, 300);
        self.kube(&[
            "rollout",
            "status",
            "deployment",
            &deployment,
            "-n",
            &spec.namespace,
            &format!("--timeout={timeout_seconds}s"),
        ])?;
        if failure == ActivationFailurePoint::AfterStageReady {
            return Err(interrupted("after Kubernetes stage became ready"));
        }
        let rollback_plan = current_plan.clone();
        let annotations = rollback_plan
            .as_ref()
            .map(|value| BTreeMap::from([("anasemble.rollback-plan", value)]));
        let service = serde_json::json!({"apiVersion":"v1","kind":"Service","metadata":{"name":spec.service,"namespace":spec.namespace,"annotations":annotations},"spec":{"selector":{"anasemble.service":spec.service,"anasemble.plan":plan_label},"ports":[{"name":"http","port":spec.service_port,"targetPort":spec.container_port}]}});
        self.kube_apply(&service)?;
        if failure == ActivationFailurePoint::AfterServiceSwitched {
            return Err(interrupted("after Kubernetes Service selector switched"));
        }
        self.kube_delete(&spec.namespace, "lease", &lease)?;
        Ok(kube_receipt(spec, approval, rollback_plan.is_some(), false))
    }
    pub fn rollback(&self, namespace: &str, service: &str) -> Result<(), Error> {
        validate_kube_name(namespace)?;
        validate_kube_name(service)?;
        let current = self.kube_json(namespace, "service", service)?;
        let rollback = current
            .pointer("/metadata/annotations/anasemble.rollback-plan")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid("Kubernetes rollback is unavailable"))?;
        let current_plan = current
            .pointer("/spec/selector/anasemble.plan")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid("Kubernetes active plan is unavailable"))?;
        let lease = format!("{service}-anasemble-lease");
        self.acquire_operation_lease(namespace, &lease, &format!("rollback:{current_plan}"))?;
        let patch = serde_json::json!({"spec":{"selector":{"anasemble.service":service,"anasemble.plan":rollback}},"metadata":{"annotations":{"anasemble.rollback-plan":null}}});
        self.kube_patch(namespace, "service", service, &patch)?;
        self.kube_delete(namespace, "lease", &lease)
    }
    pub fn commit(&self, namespace: &str, service: &str) -> Result<(), Error> {
        validate_kube_name(namespace)?;
        validate_kube_name(service)?;
        let current = self.kube_json(namespace, "service", service)?;
        let current_plan = current
            .pointer("/spec/selector/anasemble.plan")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid("Kubernetes active plan is unavailable"))?;
        let lease = format!("{service}-anasemble-lease");
        self.acquire_operation_lease(namespace, &lease, &format!("commit:{current_plan}"))?;
        if let Some(rollback) = current
            .pointer("/metadata/annotations/anasemble.rollback-plan")
            .and_then(serde_json::Value::as_str)
        {
            self.kube_delete(
                namespace,
                "deployment",
                &format!("{service}-stage-{}", &rollback[..12]),
            )?;
            let patch =
                serde_json::json!({"metadata":{"annotations":{"anasemble.rollback-plan":null}}});
            self.kube_patch(namespace, "service", service, &patch)?;
        }
        self.kube_delete(namespace, "lease", &lease)
    }
    fn acquire_operation_lease(
        &self,
        namespace: &str,
        lease: &str,
        holder: &str,
    ) -> Result<(), Error> {
        if let Some(value) = self.kube_json_optional(namespace, "lease", lease)? {
            if value
                .pointer("/spec/holderIdentity")
                .and_then(serde_json::Value::as_str)
                == Some(holder)
            {
                return Ok(());
            }
            return Err(invalid("another Kubernetes operation owns the service"));
        }
        let resource = serde_json::json!({"apiVersion":"coordination.k8s.io/v1","kind":"Lease","metadata":{"name":lease,"namespace":namespace},"spec":{"holderIdentity":holder}});
        self.kube_create(&resource)
            .map_err(|_| invalid("Kubernetes operation lease acquisition raced"))
    }
    fn apply_network_policy(&self, spec: &KubernetesDeploymentSpec) -> Result<(), Error> {
        let value = serde_json::json!({"apiVersion":"networking.k8s.io/v1","kind":"NetworkPolicy","metadata":{"name":format!("{}-deny-egress",spec.service),"namespace":spec.namespace},"spec":{"podSelector":{"matchLabels":{"anasemble.service":spec.service}},"policyTypes":["Egress"],"egress":[]}});
        self.kube_apply(&value)
    }
    fn apply_deployment(&self, spec: &KubernetesDeploymentSpec, name: &str) -> Result<(), Error> {
        let plan_label = &spec.artifact.activation_plan_sha256[..63];
        let secret_volumes:Vec<_>=spec.secrets.iter().map(|secret|serde_json::json!({"name":format!("secret-{}",secret.id),"secret":{"secretName":secret.secret_name,"items":[{"key":secret.secret_key,"path":"value"}]}})).collect();
        let secret_mounts:Vec<_>=spec.secrets.iter().map(|secret|serde_json::json!({"name":format!("secret-{}",secret.id),"mountPath":secret.mount_path,"subPath":"value","readOnly":true})).collect();
        let mut volumes = secret_volumes;
        volumes.push(serde_json::json!({"name":"tmp","emptyDir":{"medium":"Memory","sizeLimit":spec.isolation.writable_tmpfs_bytes.to_string()}}));
        let mut mounts = secret_mounts;
        mounts.push(serde_json::json!({"name":"tmp","mountPath":"/tmp"}));
        let value = serde_json::json!({"apiVersion":"apps/v1","kind":"Deployment","metadata":{"name":name,"namespace":spec.namespace,"labels":{"anasemble.service":spec.service,"anasemble.plan":plan_label}},"spec":{"replicas":1,"strategy":{"type":"Recreate"},"selector":{"matchLabels":{"anasemble.service":spec.service,"anasemble.plan":plan_label}},"template":{"metadata":{"labels":{"anasemble.service":spec.service,"anasemble.plan":plan_label}},"spec":{"automountServiceAccountToken":false,"securityContext":{"runAsNonRoot":true,"runAsUser":65534,"runAsGroup":65534,"seccompProfile":{"type":"RuntimeDefault"}},"containers":[{"name":"candidate","image":spec.artifact.immutable_image,"imagePullPolicy":"IfNotPresent","command":spec.command,"ports":[{"containerPort":spec.container_port}],"resources":{"requests":{"cpu":format!("{}m",spec.isolation.cpu_millis),"memory":spec.isolation.memory_bytes.to_string()},"limits":{"cpu":format!("{}m",spec.isolation.cpu_millis),"memory":spec.isolation.memory_bytes.to_string()}},"securityContext":{"allowPrivilegeEscalation":false,"readOnlyRootFilesystem":true,"capabilities":{"drop":["ALL"],"add":spec.isolation.linux_capabilities}},"volumeMounts":mounts,"readinessProbe":{"exec":{"command":spec.health.command},"periodSeconds":((spec.health.interval_ms/1_000).max(1)),"failureThreshold":spec.health.attempts}}],"volumes":volumes}}}});
        self.kube_apply(&value)
    }
    fn kube(&self, args: &[&str]) -> Result<(), Error> {
        let mut command = vec!["--context", &self.context];
        command.extend_from_slice(args);
        let output = Command::new("kubectl")
            .args(command)
            .output()
            .map_err(Error::Io)?;
        if output.stdout.len().saturating_add(output.stderr.len()) > MAX_DOCKER_OUTPUT {
            return Err(invalid("Kubernetes output exceeded 1 MiB"));
        }
        if !output.status.success() {
            return Err(invalid(&format!(
                "Kubernetes operation failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(())
    }
    fn kube_apply(&self, value: &serde_json::Value) -> Result<(), Error> {
        self.kube_input(&["apply", "-f", "-"], value)
    }
    fn kube_create(&self, value: &serde_json::Value) -> Result<(), Error> {
        self.kube_input(&["create", "-f", "-"], value)
    }
    fn kube_input(&self, args: &[&str], value: &serde_json::Value) -> Result<(), Error> {
        use std::io::Write as _;
        use std::process::Stdio;
        let mut command = Command::new("kubectl");
        command
            .args(["--context", &self.context])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(Error::Io)?;
        child
            .stdin
            .take()
            .ok_or_else(|| invalid("kubectl stdin unavailable"))?
            .write_all(&encode(value)?)
            .map_err(Error::Io)?;
        let output = child.wait_with_output().map_err(Error::Io)?;
        if output.stdout.len().saturating_add(output.stderr.len()) > MAX_DOCKER_OUTPUT {
            return Err(invalid("Kubernetes output exceeded 1 MiB"));
        }
        if !output.status.success() {
            return Err(invalid(&format!(
                "Kubernetes apply failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(())
    }
    fn kube_json_optional(
        &self,
        namespace: &str,
        kind: &str,
        name: &str,
    ) -> Result<Option<serde_json::Value>, Error> {
        let output = Command::new("kubectl")
            .args([
                "--context",
                &self.context,
                "get",
                kind,
                name,
                "-n",
                namespace,
                "--ignore-not-found=true",
                "-o",
                "json",
            ])
            .output()
            .map_err(Error::Io)?;
        if !output.status.success() {
            return Err(invalid(&format!(
                "Kubernetes read failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        if output.stdout.len().saturating_add(output.stderr.len()) > MAX_DOCKER_OUTPUT {
            return Err(invalid("Kubernetes object exceeded 1 MiB"));
        }
        if output.stdout.is_empty() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_slice(&output.stdout)?))
    }
    fn kube_json(
        &self,
        namespace: &str,
        kind: &str,
        name: &str,
    ) -> Result<serde_json::Value, Error> {
        self.kube_json_optional(namespace, kind, name)?
            .ok_or_else(|| invalid("Kubernetes object does not exist"))
    }
    fn kube_patch(
        &self,
        namespace: &str,
        kind: &str,
        name: &str,
        patch: &serde_json::Value,
    ) -> Result<(), Error> {
        let patch = serde_json::to_string(patch)?;
        self.kube(&[
            "patch",
            kind,
            name,
            "-n",
            namespace,
            "--type=merge",
            "-p",
            &patch,
        ])
    }
    fn kube_delete(&self, namespace: &str, kind: &str, name: &str) -> Result<(), Error> {
        self.kube(&[
            "delete",
            kind,
            name,
            "-n",
            namespace,
            "--ignore-not-found=true",
            "--wait=true",
            "--timeout=30s",
        ])
    }
}

fn validate_kubernetes_deployment(spec: &KubernetesDeploymentSpec) -> Result<(), Error> {
    if spec.version != "kubernetes-deployment-v1" {
        return Err(invalid("Kubernetes deployment version is unsupported"));
    }
    validate_kube_name(&spec.namespace)?;
    validate_kube_name(&spec.service)?;
    validate_arguments(&spec.command)?;
    spec.isolation.validate()?;
    if spec.service_port == 0 || spec.container_port == 0 {
        return Err(invalid("Kubernetes service ports must be non-zero"));
    }
    if spec.health.attempts == 0 || spec.health.attempts > 120 || spec.health.interval_ms > 10_000 {
        return Err(invalid("Kubernetes health bounds are invalid"));
    }
    validate_arguments(&spec.health.command)?;
    if spec.secrets.len() > 32 {
        return Err(invalid("Kubernetes secret count exceeds 32"));
    }
    let mut ids = BTreeSet::new();
    for secret in &spec.secrets {
        validate_kube_name(&secret.id)?;
        validate_kube_name(&secret.secret_name)?;
        validate_kube_name(&secret.secret_key)?;
        if !ids.insert(&secret.id)
            || !secret.mount_path.starts_with("/run/secrets/")
            || secret.mount_path.contains("..")
        {
            return Err(invalid("Kubernetes secret reference is invalid"));
        }
    }
    let fake = DeploymentSpec {
        version: "docker-deployment-v1".into(),
        service: spec.service.clone(),
        artifact: spec.artifact.clone(),
        command: spec.command.clone(),
        isolation: spec.isolation.clone(),
        secrets: Vec::new(),
        health: spec.health.clone(),
    };
    validate_deployment(&fake)
}
fn validate_kube_name(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 63
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (byte == b'-' && index > 0 && index + 1 < value.len())
        })
    {
        return Err(invalid("Kubernetes name is invalid"));
    }
    Ok(())
}
fn kube_receipt(
    spec: &KubernetesDeploymentSpec,
    approval: &OperatorApproval,
    rollback_available: bool,
    idempotent: bool,
) -> ActivationReceipt {
    ActivationReceipt {
        service: spec.service.clone(),
        plan_sha256: spec.artifact.activation_plan_sha256.clone(),
        immutable_image: spec.artifact.immutable_image.clone(),
        operator_key_id: approval.operator_key_id.clone(),
        secret_reference_ids: spec
            .secrets
            .iter()
            .map(|secret| secret.id.clone())
            .collect(),
        internal_network: format!("networkpolicy/{}-deny-egress", spec.service),
        rollback_available,
        idempotent,
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ActivationReceipt {
    pub service: String,
    pub plan_sha256: String,
    pub immutable_image: String,
    pub operator_key_id: String,
    pub secret_reference_ids: Vec<String>,
    pub internal_network: String,
    pub rollback_available: bool,
    pub idempotent: bool,
}

pub struct DockerOrchestrator {
    approval_policy: ApprovalPolicy,
}

impl DockerOrchestrator {
    pub fn new(approval_policy: ApprovalPolicy) -> Self {
        Self { approval_policy }
    }

    pub fn activate(
        &self,
        spec: &DeploymentSpec,
        approval: &OperatorApproval,
    ) -> Result<ActivationReceipt, Error> {
        self.activate_with_failure(spec, approval, ActivationFailurePoint::None)
    }

    pub fn activate_with_failure(
        &self,
        spec: &DeploymentSpec,
        approval: &OperatorApproval,
        failure: ActivationFailurePoint,
    ) -> Result<ActivationReceipt, Error> {
        validate_deployment(spec)?;
        self.approval_policy.verify(
            approval,
            &spec.artifact.activation_plan_sha256,
            &spec.artifact.binding_sha256,
        )?;
        let names = DeploymentNames::new(&spec.service, &spec.artifact.activation_plan_sha256)?;
        ensure_internal_network(&names.network, &spec.service)?;
        let lease_created = acquire_lease(
            &names.lease,
            &format!("activate:{}", spec.artifact.activation_plan_sha256),
            &spec.artifact.immutable_image,
        )?;
        let active_plan = container_label(&names.active, "anasemble.plan")?;
        if active_plan.as_deref() == Some(&spec.artifact.activation_plan_sha256) {
            remove_if_exists(&names.stage)?;
            remove_if_exists(&names.lease)?;
            return Ok(receipt(
                spec,
                approval,
                container_exists(&names.rollback)?,
                true,
            ));
        }
        if container_exists(&names.rollback)? && !container_exists(&names.stage)? {
            if lease_created {
                remove_if_exists(&names.lease)?;
            }
            return Err(invalid(
                "stale rollback container requires commit or rollback",
            ));
        }
        if !container_exists(&names.stage)? {
            create_staged_container(spec, &names.stage)?;
            docker(&["start".into(), names.stage.clone()])?;
            if let Err(error) = wait_healthy(&names.stage, &spec.health) {
                remove_if_exists(&names.stage)?;
                remove_if_exists(&names.lease)?;
                return Err(error);
            }
        }
        if failure == ActivationFailurePoint::AfterStageHealthy {
            return Err(interrupted("after staged health passed"));
        }
        let had_active = container_exists(&names.active)?;
        if had_active {
            if inspect_bool(&names.active, "{{.State.Running}}")? {
                docker(&[
                    "stop".into(),
                    "--time".into(),
                    "5".into(),
                    names.active.clone(),
                ])?;
            }
            if failure == ActivationFailurePoint::AfterActiveStopped {
                return Err(interrupted("after active container stopped"));
            }
            if !container_exists(&names.rollback)? {
                docker(&[
                    "rename".into(),
                    names.active.clone(),
                    names.rollback.clone(),
                ])?;
            }
        }
        if failure == ActivationFailurePoint::AfterActiveRenamed {
            return Err(interrupted("after active container renamed for rollback"));
        }
        if container_exists(&names.active)? {
            return Err(invalid("activation target unexpectedly exists"));
        }
        docker(&["rename".into(), names.stage.clone(), names.active.clone()])?;
        if !inspect_bool(&names.active, "{{.State.Running}}")? {
            docker(&["start".into(), names.active.clone()])?;
        }
        remove_if_exists(&names.lease)?;
        Ok(receipt(
            spec,
            approval,
            had_active || container_exists(&names.rollback)?,
            false,
        ))
    }

    pub fn rollback(&self, service: &str, plan_sha256: &str) -> Result<(), Error> {
        let names = DeploymentNames::new(service, plan_sha256)?;
        if !container_exists(&names.rollback)? {
            return Err(invalid("deployment rollback is unavailable"));
        }
        let active_image = container_image(&names.active)?
            .ok_or_else(|| invalid("active deployment image is unavailable"))?;
        acquire_lease(
            &names.lease,
            &format!("rollback:{plan_sha256}"),
            &active_image,
        )?;
        if container_exists(&names.active)? {
            if inspect_bool(&names.active, "{{.State.Running}}")? {
                docker(&[
                    "stop".into(),
                    "--time".into(),
                    "5".into(),
                    names.active.clone(),
                ])?;
            }
            remove_if_exists(&names.failed)?;
            docker(&["rename".into(), names.active.clone(), names.failed.clone()])?;
        }
        docker(&[
            "rename".into(),
            names.rollback.clone(),
            names.active.clone(),
        ])?;
        docker(&["start".into(), names.active.clone()])?;
        remove_if_exists(&names.lease)?;
        Ok(())
    }

    pub fn commit(&self, service: &str, plan_sha256: &str) -> Result<(), Error> {
        let names = DeploymentNames::new(service, plan_sha256)?;
        if container_label(&names.active, "anasemble.plan")?.as_deref() != Some(plan_sha256) {
            return Err(invalid("cannot commit a different active deployment"));
        }
        let active_image = container_image(&names.active)?
            .ok_or_else(|| invalid("active deployment image is unavailable"))?;
        acquire_lease(
            &names.lease,
            &format!("commit:{plan_sha256}"),
            &active_image,
        )?;
        remove_if_exists(&names.rollback)?;
        remove_if_exists(&names.failed)?;
        remove_if_exists(&names.lease)
    }
}

struct DeploymentNames {
    active: String,
    stage: String,
    rollback: String,
    failed: String,
    lease: String,
    network: String,
}
impl DeploymentNames {
    fn new(service: &str, plan: &str) -> Result<Self, Error> {
        validate_label(service)?;
        validate_digest(plan)?;
        let short = &plan[..12];
        Ok(Self {
            active: format!("anasemble-{service}-active"),
            stage: format!("anasemble-{service}-stage-{short}"),
            rollback: format!("anasemble-{service}-rollback"),
            failed: format!("anasemble-{service}-failed"),
            lease: format!("anasemble-{service}-lease"),
            network: format!("anasemble-{service}-network"),
        })
    }
}

fn validate_deployment(spec: &DeploymentSpec) -> Result<(), Error> {
    if spec.version != "docker-deployment-v1" {
        return Err(invalid("deployment version is unsupported"));
    }
    validate_label(&spec.service)?;
    validate_digest(&spec.artifact.activation_plan_sha256)?;
    validate_digest(&spec.artifact.candidate_sha256)?;
    validate_digest(&spec.artifact.manifest_sha256)?;
    validate_digest(&spec.artifact.binding_sha256)?;
    validate_immutable_image(&spec.artifact.immutable_image)?;
    if !spec
        .artifact
        .immutable_image
        .ends_with(&format!("@sha256:{}", spec.artifact.manifest_sha256))
    {
        return Err(invalid("deployment artifact digest is inconsistent"));
    }
    let mut unsigned = spec.artifact.clone();
    let expected = std::mem::take(&mut unsigned.binding_sha256);
    if crate::canonical::digest(&unsigned)? != expected {
        return Err(invalid("deployment artifact binding is invalid"));
    }
    validate_arguments(&spec.command)?;
    spec.isolation.validate()?;
    if spec.health.attempts == 0 || spec.health.attempts > 120 || spec.health.interval_ms > 10_000 {
        return Err(invalid("health probe bounds are invalid"));
    }
    validate_arguments(&spec.health.command)?;
    if spec.secrets.len() > 32 {
        return Err(invalid("secret reference count exceeds 32"));
    }
    let mut ids = BTreeSet::new();
    let mut mounts = BTreeSet::new();
    for secret in &spec.secrets {
        secret.validate()?;
        if !ids.insert(&secret.id) || !mounts.insert(&secret.mount_path) {
            return Err(invalid("secret references must have unique IDs and mounts"));
        }
    }
    Ok(())
}

fn create_staged_container(spec: &DeploymentSpec, name: &str) -> Result<(), Error> {
    let network = format!("anasemble-{}-network", spec.service);
    let mut args = vec![
        "create".into(),
        "--name".into(),
        name.into(),
        "--label".into(),
        format!("anasemble.plan={}", spec.artifact.activation_plan_sha256),
        "--label".into(),
        format!("anasemble.service={}", spec.service),
        "--network".into(),
        network,
        "--read-only".into(),
        "--cap-drop".into(),
        "ALL".into(),
        "--security-opt".into(),
        "no-new-privileges".into(),
        "--pids-limit".into(),
        spec.isolation.pids.to_string(),
        "--memory".into(),
        spec.isolation.memory_bytes.to_string(),
        "--memory-swap".into(),
        spec.isolation.memory_bytes.to_string(),
        "--cpus".into(),
        format!("{:.3}", f64::from(spec.isolation.cpu_millis) / 1_000.0),
        "--tmpfs".into(),
        format!(
            "/tmp:rw,noexec,nosuid,nodev,size={}",
            spec.isolation.writable_tmpfs_bytes
        ),
        "--log-driver".into(),
        "none".into(),
    ];
    for cap in &spec.isolation.linux_capabilities {
        args.extend(["--cap-add".into(), cap.clone()]);
    }
    for secret in &spec.secrets {
        let source = secret.source_file.canonicalize().map_err(Error::Io)?;
        args.extend([
            "--mount".into(),
            format!(
                "type=bind,src={},dst={},readonly",
                source.display(),
                secret.mount_path
            ),
        ]);
    }
    args.push(spec.artifact.immutable_image.clone());
    args.extend(spec.command.clone());
    docker(&args)?;
    Ok(())
}

fn wait_healthy(name: &str, probe: &HealthProbe) -> Result<(), Error> {
    for attempt in 0..probe.attempts {
        let mut args = vec!["exec".into(), name.into()];
        args.extend(probe.command.clone());
        if docker_raw(&args).is_ok() {
            return Ok(());
        }
        if attempt + 1 < probe.attempts {
            thread::sleep(Duration::from_millis(probe.interval_ms));
        }
    }
    Err(invalid("staged deployment failed its bounded health gate"))
}

fn acquire_lease(name: &str, plan: &str, image: &str) -> Result<bool, Error> {
    if container_exists(name)? {
        if container_label(name, "anasemble.plan")?.as_deref() == Some(plan) {
            return Ok(false);
        }
        return Err(invalid("another activation lease owns the service"));
    }
    docker(&[
        "create".into(),
        "--name".into(),
        name.into(),
        "--label".into(),
        format!("anasemble.plan={plan}"),
        "--network".into(),
        "none".into(),
        image.into(),
        "/bin/true".into(),
    ])?;
    Ok(true)
}

fn container_image(name: &str) -> Result<Option<String>, Error> {
    if !container_exists(name)? {
        return Ok(None);
    }
    Ok(Some(
        docker_text(&[
            "inspect".into(),
            "--format".into(),
            "{{.Config.Image}}".into(),
            name.into(),
        ])?
        .trim()
        .to_owned(),
    ))
}

fn receipt(
    spec: &DeploymentSpec,
    approval: &OperatorApproval,
    rollback_available: bool,
    idempotent: bool,
) -> ActivationReceipt {
    ActivationReceipt {
        service: spec.service.clone(),
        plan_sha256: spec.artifact.activation_plan_sha256.clone(),
        immutable_image: spec.artifact.immutable_image.clone(),
        operator_key_id: approval.operator_key_id.clone(),
        secret_reference_ids: spec
            .secrets
            .iter()
            .map(|secret| secret.id.clone())
            .collect(),
        internal_network: format!("anasemble-{}-network", spec.service),
        rollback_available,
        idempotent,
    }
}

fn ensure_internal_network(name: &str, service: &str) -> Result<(), Error> {
    let output = Command::new("docker")
        .args(["network", "inspect", name])
        .output()
        .map_err(Error::Io)?;
    if output.status.success() {
        let internal = docker_text(&[
            "network".into(),
            "inspect".into(),
            "--format".into(),
            "{{.Internal}}".into(),
            name.into(),
        ])?;
        if internal.trim() != "true" {
            return Err(invalid(
                "service network exists without internal egress isolation",
            ));
        }
        return Ok(());
    }
    docker(&[
        "network".into(),
        "create".into(),
        "--internal".into(),
        "--label".into(),
        format!("anasemble.service={service}"),
        name.into(),
    ])?;
    Ok(())
}

fn validate_immutable_image(image: &str) -> Result<(), Error> {
    let Some((_, digest)) = image.rsplit_once("@sha256:") else {
        return Err(invalid(
            "container image must use an immutable sha256 digest",
        ));
    };
    validate_digest(digest)
}
fn validate_arguments(args: &[String]) -> Result<(), Error> {
    if args.is_empty()
        || args.len() > MAX_ARGUMENTS
        || args.iter().map(String::len).sum::<usize>() > MAX_ARGUMENT_BYTES
        || args
            .iter()
            .any(|value| value.is_empty() || value.contains('\0'))
    {
        return Err(invalid(
            "container command is empty or exceeds argument bounds",
        ));
    }
    Ok(())
}
fn validate_label(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(invalid("identifier is empty, too long, or unsafe"));
    }
    Ok(())
}
fn validate_digest(value: &str) -> Result<(), Error> {
    if value.len() != 64 || hex::decode(value).map_or(true, |bytes| bytes.len() != 32) {
        return Err(invalid("expected a SHA-256 digest"));
    }
    Ok(())
}

fn docker(args: &[String]) -> Result<(), Error> {
    docker_raw(args).map(|_| ())
}
fn docker_text(args: &[String]) -> Result<String, Error> {
    String::from_utf8(docker_bytes(args)?).map_err(|_| invalid("Docker returned non-UTF-8 output"))
}
fn docker_bytes(args: &[String]) -> Result<Vec<u8>, Error> {
    let output = docker_raw(args)?;
    if output.stdout.len() > MAX_DOCKER_OUTPUT {
        return Err(invalid("Docker output exceeded 1 MiB"));
    }
    Ok(output.stdout)
}
fn docker_raw(args: &[String]) -> Result<Output, Error> {
    let output = Command::new("docker")
        .args(args)
        .output()
        .map_err(Error::Io)?;
    if output.stdout.len().saturating_add(output.stderr.len()) > MAX_DOCKER_OUTPUT {
        return Err(invalid("Docker output exceeded 1 MiB"));
    }
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(invalid(&format!(
            "Docker operation failed: {}",
            detail.trim()
        )));
    }
    Ok(output)
}
fn container_exists(name: &str) -> Result<bool, Error> {
    let status = Command::new("docker")
        .args(["container", "inspect", name])
        .output()
        .map_err(Error::Io)?;
    Ok(status.status.success())
}
fn container_label(name: &str, label: &str) -> Result<Option<String>, Error> {
    if !container_exists(name)? {
        return Ok(None);
    }
    let value = docker_text(&[
        "inspect".into(),
        "--format".into(),
        format!("{{{{index .Config.Labels \"{label}\"}}}}"),
        name.into(),
    ])?;
    let value = value.trim();
    Ok((!value.is_empty() && value != "<no value>").then(|| value.into()))
}
fn inspect_bool(name: &str, template: &str) -> Result<bool, Error> {
    match docker_text(&[
        "inspect".into(),
        "--format".into(),
        template.into(),
        name.into(),
    ])?
    .trim()
    {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(invalid("Docker inspect returned an invalid boolean")),
    }
}
fn inspect_i64(name: &str, template: &str) -> Result<i64, Error> {
    docker_text(&[
        "inspect".into(),
        "--format".into(),
        template.into(),
        name.into(),
    ])?
    .trim()
    .parse()
    .map_err(backend)
}
fn remove_if_exists(name: &str) -> Result<(), Error> {
    if container_exists(name)? {
        docker(&["rm".into(), "--force".into(), name.into()])?;
    }
    Ok(())
}
fn unique_name(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        jiff::Timestamp::now().as_nanosecond()
    )
}
fn interrupted(point: &str) -> Error {
    Error::Io(std::io::Error::other(format!(
        "injected interruption {point}"
    )))
}
fn invalid(message: &str) -> Error {
    Error::InvalidEvidence(message.into())
}
fn backend(error: impl std::fmt::Display) -> Error {
    Error::InvalidEvidence(format!("activation backend error: {error}"))
}

struct ContainerGuard {
    name: String,
}
impl ContainerGuard {
    fn new(name: String) -> Self {
        Self { name }
    }
}
impl Drop for ContainerGuard {
    fn drop(&mut self) {
        if self.name.starts_with("anasemble-sandbox-") {
            let _ = Command::new("docker")
                .args(["rm", "--force", &self.name])
                .output();
        }
    }
}
