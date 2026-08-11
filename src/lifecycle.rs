//! Exact-prefix installation and removal for the supported local binary profile.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::canonical::{bytes_digest, encode};
use crate::model::Error;
use crate::operations::OperationsConfig;

const MAX_INSTALL_FILE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityManifest {
    pub version: String,
    pub product_version: String,
    pub rust_version: String,
    pub status_definitions: CompatibilityStatusDefinitions,
    pub configuration_versions: Vec<String>,
    pub recovery_protocols: Vec<String>,
    pub profiles: Vec<CompatibilityProfile>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityStatusDefinitions {
    pub implementation: String,
    pub validation: String,
    pub support: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationStatus {
    Implemented,
    Partial,
    NotImplemented,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Tested,
    PartiallyTested,
    Untested,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SupportStatus {
    Supported,
    Experimental,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityProfile {
    pub id: String,
    pub platform: String,
    pub architecture: String,
    pub components: Vec<String>,
    pub transport: String,
    pub implementation: ImplementationStatus,
    pub validation: ValidationStatus,
    pub support: SupportStatus,
    pub evidence: Vec<String>,
    pub limitations: Vec<String>,
}

impl Default for CompatibilityManifest {
    fn default() -> Self {
        Self {
            version: "compatibility-v2".into(),
            product_version: env!("CARGO_PKG_VERSION").into(),
            rust_version: "1.97.0".into(),
            status_definitions: CompatibilityStatusDefinitions {
                implementation: "implemented means the bounded code path exists; partial means only part of the named profile exists; not_implemented means it does not exist".into(),
                validation: "tested means retained evidence exercises the exact profile; partially_tested means only named boundaries were exercised; untested means no retained execution evidence exists".into(),
                support: "supported means defects are accepted for the exact profile; experimental permits evaluation without that commitment; unsupported must be refused".into(),
            },
            configuration_versions: vec![
                "operations-config-v0-migratable".into(),
                "operations-config-v1".into(),
            ],
            recovery_protocols: vec!["fsm-v1".into(), "service-v1".into()],
            profiles: vec![
                profile("macos-arm64-control-plane", "macos", "aarch64", &["control-plane", "filesystem-v1", "operations-v1"], "local-filesystem", ImplementationStatus::Implemented, ValidationStatus::Tested, SupportStatus::Supported, &["scripts/ci-local.sh", "tests/p4_product_readiness.rs"], &["requires the Rust 1.97.0 locked dependency graph"]),
                profile("macos-arm64-p2-local-state", "macos", "aarch64", &["postgresql-18", "minio-s3-api", "redis-stream-8.8"], "trusted-loopback", ImplementationStatus::Implemented, ValidationStatus::Tested, SupportStatus::Supported, &["tests/p2_stateful.rs", "docs/P2_STATEFUL_RECOVERY.md"], &["writers must be quiesced", "no cross-backend transaction"]),
                profile("macos-arm64-docker-activation", "macos", "aarch64", &["oci-distribution-v2", "docker-engine-29"], "local-docker-daemon", ImplementationStatus::Implemented, ValidationStatus::Tested, SupportStatus::Supported, &["tests/p3_activation.rs", "docs/P3_ISOLATED_ACTIVATION.md"], &["single host", "Docker daemon and host kernel are trusted"]),
                profile("macos-arm64-kind-kubernetes-activation", "macos", "aarch64", &["kubernetes-1.36", "kubectl-1.36", "kind-0.32"], "local-kind-cluster", ImplementationStatus::Implemented, ValidationStatus::PartiallyTested, SupportStatus::Experimental, &["tests/p3_activation.rs", "docs/P3_ISOLATED_ACTIVATION.md"], &["control objects and switching are tested", "NetworkPolicy enforcement is not tested"]),
                profile("linux-arm64-control-plane", "linux-gnu", "aarch64", &["control-plane"], "local-container", ImplementationStatus::Implemented, ValidationStatus::Tested, SupportStatus::Experimental, &["scripts/ci-linux-matrix.sh", "docs/LINUX_MATRIX.md"], &["tested in a clean Debian-based Linux container on an arm64 Docker host", "production distribution and kernel combinations remain unverified"]),
                profile("linux-x86_64-control-plane", "linux-gnu", "x86_64", &["control-plane"], "emulated-local-container", ImplementationStatus::Implemented, ValidationStatus::PartiallyTested, SupportStatus::Experimental, &["scripts/ci-linux-matrix.sh", "docs/LINUX_MATRIX.md"], &["built and executed under Docker amd64 emulation on an arm64 host", "native x86_64 hardware and production distribution combinations remain unverified"]),
                profile("aws-al2023-arm64-control-plane", "amazon-linux-2023", "aarch64", &["control-plane"], "native-ec2", ImplementationStatus::Implemented, ValidationStatus::Tested, SupportStatus::Supported, &["docs/AWS_COMPATIBILITY.md"], &["validated on ami-053d8df569ac57bbb, t4g.medium, and kernel 6.1.177-224.371.amzn2023.aarch64", "other AMIs, kernels, and instance families are not implied"]),
                profile("aws-al2023-x86_64-control-plane", "amazon-linux-2023", "x86_64", &["control-plane"], "native-ec2", ImplementationStatus::Implemented, ValidationStatus::Tested, SupportStatus::Supported, &["docs/AWS_COMPATIBILITY.md"], &["validated on ami-062a8901a5ddcf280, t3.medium, and kernel 6.1.177-224.371.amzn2023.x86_64", "other AMIs, kernels, and instance families are not implied"]),
                profile("aws-managed-state-eu-west-1", "amazon-linux-2023", "aarch64", &["rds-postgresql-18.3", "amazon-s3", "elasticache-redis-7.1"], "private-tls-and-https", ImplementationStatus::Implemented, ValidationStatus::Tested, SupportStatus::Supported, &["tests/aws_compatibility.rs", "docs/AWS_COMPATIBILITY.md"], &["eu-west-1 provider profile only", "writers must be quiesced", "no cross-backend transaction", "ElastiCache server-authenticated TLS and AUTH are trusted without client certificates"]),
                profile("aws-eks-1.36-vpc-cni", "amazon-eks", "aarch64", &["kubernetes-1.36", "eks-platform-eks.9", "vpc-cni-1.22.4", "networkpolicy-strict"], "authenticated-kubernetes-api", ImplementationStatus::Implemented, ValidationStatus::Tested, SupportStatus::Supported, &["tests/aws_compatibility.rs", "docs/AWS_COMPATIBILITY.md"], &["one AL2023_ARM_64_STANDARD t4g.medium node", "EKS control plane, VPC CNI, admission, IAM, and cluster administrators remain trusted"]),
                profile("generic-s3-compatible-https", "provider-managed", "provider-dependent", &["s3-compatible-object-store"], "https", ImplementationStatus::Implemented, ValidationStatus::PartiallyTested, SupportStatus::Experimental, &["tests/p2_stateful.rs"], &["only the MinIO S3 API fixture is tested", "provider-specific behavior is unverified"]),
                profile("other-production-kubernetes-enforcing-cni", "linux-gnu", "provider-dependent", &["kubernetes-1.36", "networkpolicy-enforcing-cni"], "authenticated-kubernetes-api", ImplementationStatus::Implemented, ValidationStatus::PartiallyTested, SupportStatus::Experimental, &["tests/p3_activation.rs", "docs/AWS_COMPATIBILITY.md"], &["the exact EKS and VPC CNI profile is validated", "no other provider or CNI is implied"]),
                profile("integrated-recovery-to-activation", "macos", "aarch64", &["reconstruction", "postgresql-18", "minio-s3-api", "redis-stream-8.8", "oci-distribution-v2", "kubernetes-1.36", "public-reference-cli"], "mixed-local", ImplementationStatus::Implemented, ValidationStatus::Tested, SupportStatus::Experimental, &["tests/reference_workflow.rs", "docs/QUICKSTART.md"], &["the packaged finite-state candidate is health-checked as an artifact and is not a generated HTTP server", "kind control objects are tested but NetworkPolicy enforcement is not", "rollback evidence must be retained until operator acceptance"]),
                profile("generic-remote-postgresql-or-redis", "provider-managed", "provider-dependent", &["postgresql", "redis-stream"], "authenticated-tls", ImplementationStatus::Implemented, ValidationStatus::PartiallyTested, SupportStatus::Experimental, &["tests/aws_compatibility.rs", "docs/P2_STATEFUL_RECOVERY.md"], &["only the exact AWS managed-state profile is supported", "other providers, versions, certificate policies, and authentication modes require retained evidence"]),
            ],
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn profile(
    id: &str,
    platform: &str,
    architecture: &str,
    components: &[&str],
    transport: &str,
    implementation: ImplementationStatus,
    validation: ValidationStatus,
    support: SupportStatus,
    evidence: &[&str],
    limitations: &[&str],
) -> CompatibilityProfile {
    CompatibilityProfile {
        id: id.into(),
        platform: platform.into(),
        architecture: architecture.into(),
        components: components.iter().map(|value| (*value).into()).collect(),
        transport: transport.into(),
        implementation,
        validation,
        support,
        evidence: evidence.iter().map(|value| (*value).into()).collect(),
        limitations: limitations.iter().map(|value| (*value).into()).collect(),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InstalledFile {
    pub relative_path: String,
    pub sha256: String,
    pub mode: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InstallManifest {
    pub version: String,
    pub product_version: String,
    pub files: Vec<InstalledFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InstallReceipt {
    pub prefix: PathBuf,
    pub product_version: String,
    pub installed_files: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UninstallReceipt {
    pub prefix: PathBuf,
    pub removed_files: usize,
    pub removed_prefix: bool,
}

pub fn install(prefix: &Path) -> Result<InstallReceipt, Error> {
    if prefix.try_exists()? {
        return Err(invalid("installation prefix already exists"));
    }
    let parent = prefix
        .parent()
        .ok_or_else(|| invalid("installation prefix requires a parent"))?;
    validate_directory(parent)?;
    let name = prefix
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("installation prefix requires a UTF-8 name"))?;
    let staging = parent.join(format!(".{name}.anasemble-install-{}", std::process::id()));
    if staging.try_exists()? {
        return Err(invalid(
            "stale installation staging directory requires operator action",
        ));
    }
    fs::create_dir(&staging)?;
    let mut guard = InstallGuard::new(staging.clone());
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o755))?;
    let bin = staging.join("bin");
    let share = staging.join("share");
    fs::create_dir(&bin)?;
    fs::create_dir(&share)?;
    let executable = std::env::current_exe()?;
    let executable_bytes = read_regular(&executable, MAX_INSTALL_FILE_BYTES)?;
    let compatibility = encode(&CompatibilityManifest::default())?;
    let config = encode(&OperationsConfig::default())?;
    let payloads = [
        ("bin/anasemble", executable_bytes.as_slice(), 0o755),
        (
            "share/compatibility-v2.json",
            compatibility.as_slice(),
            0o644,
        ),
        ("share/operations-config-v1.json", config.as_slice(), 0o644),
    ];
    let mut files = Vec::new();
    for (relative, bytes, mode) in payloads {
        let path = staging.join(relative);
        guard.files.push(path.clone());
        write_new(&path, bytes, mode)?;
        files.push(InstalledFile {
            relative_path: relative.into(),
            sha256: bytes_digest(bytes),
            mode,
        });
    }
    let manifest = InstallManifest {
        version: "install-manifest-v1".into(),
        product_version: env!("CARGO_PKG_VERSION").into(),
        files,
    };
    let manifest_path = staging.join("install-manifest-v1.json");
    guard.files.push(manifest_path.clone());
    write_new(&manifest_path, &encode(&manifest)?, 0o644)?;
    File::open(&staging)?.sync_all()?;
    fs::rename(&staging, prefix)?;
    File::open(parent)?.sync_all()?;
    guard.committed = true;
    Ok(InstallReceipt {
        prefix: prefix.into(),
        product_version: manifest.product_version,
        installed_files: manifest.files.len() + 1,
    })
}

struct InstallGuard {
    root: PathBuf,
    files: Vec<PathBuf>,
    committed: bool,
}

impl InstallGuard {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            files: Vec::new(),
            committed: false,
        }
    }
}

impl Drop for InstallGuard {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for file in self.files.iter().rev() {
            let _ = fs::remove_file(file);
        }
        let _ = fs::remove_dir(self.root.join("bin"));
        let _ = fs::remove_dir(self.root.join("share"));
        let _ = fs::remove_dir(&self.root);
    }
}

pub fn uninstall(prefix: &Path) -> Result<UninstallReceipt, Error> {
    validate_directory(prefix)?;
    validate_exact_entries(prefix, &["bin", "install-manifest-v1.json", "share"])?;
    validate_directory(&prefix.join("bin"))?;
    validate_directory(&prefix.join("share"))?;
    validate_exact_entries(&prefix.join("bin"), &["anasemble"])?;
    validate_exact_entries(
        &prefix.join("share"),
        &["compatibility-v2.json", "operations-config-v1.json"],
    )?;
    let manifest_path = prefix.join("install-manifest-v1.json");
    let manifest_bytes = read_regular(&manifest_path, 65_536)?;
    let manifest: InstallManifest = serde_json::from_slice(&manifest_bytes)?;
    if manifest.version != "install-manifest-v1"
        || manifest.files.len() != 3
        || manifest.product_version != env!("CARGO_PKG_VERSION")
    {
        return Err(invalid("installation manifest is unsupported"));
    }
    for file in &manifest.files {
        validate_relative(&file.relative_path)?;
        let path = prefix.join(&file.relative_path);
        let bytes = read_regular(&path, MAX_INSTALL_FILE_BYTES)?;
        if bytes_digest(&bytes) != file.sha256 {
            return Err(invalid("installed file changed; uninstallation refused"));
        }
        if fs::metadata(path)?.permissions().mode() & 0o777 != file.mode {
            return Err(invalid(
                "installed file mode changed; uninstallation refused",
            ));
        }
    }
    for file in &manifest.files {
        fs::remove_file(prefix.join(&file.relative_path))?;
    }
    fs::remove_file(&manifest_path)?;
    fs::remove_dir(prefix.join("bin"))?;
    fs::remove_dir(prefix.join("share"))?;
    fs::remove_dir(prefix)?;
    File::open(
        prefix
            .parent()
            .ok_or_else(|| invalid("installation prefix requires a parent"))?,
    )?
    .sync_all()?;
    Ok(UninstallReceipt {
        prefix: prefix.into(),
        removed_files: 4,
        removed_prefix: true,
    })
}

fn validate_relative(value: &str) -> Result<(), Error> {
    let path = Path::new(value);
    if value.is_empty()
        || value.len() > 128
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(invalid("installation manifest path is invalid"));
    }
    Ok(())
}

fn validate_directory(path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid("installation parent must be a real directory"));
    }
    Ok(())
}

fn validate_exact_entries(directory: &Path, expected: &[&str]) -> Result<(), Error> {
    let mut actual = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        actual.push(
            entry
                .file_name()
                .into_string()
                .map_err(|_| invalid("installation contains a non-UTF-8 entry"))?,
        );
    }
    actual.sort();
    let mut expected: Vec<_> = expected.iter().map(|value| (*value).to_owned()).collect();
    expected.sort();
    if actual != expected {
        return Err(invalid(
            "installation topology differs from its owned manifest",
        ));
    }
    Ok(())
}

fn read_regular(path: &Path, max: u64) -> Result<Vec<u8>, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > max {
        return Err(invalid(
            "installation file is invalid or exceeds its byte bound",
        ));
    }
    Ok(fs::read(path)?)
}

fn write_new(path: &Path, bytes: &[u8], mode: u32) -> Result<(), Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn invalid(message: &str) -> Error {
    Error::InvalidEvidence(message.into())
}
