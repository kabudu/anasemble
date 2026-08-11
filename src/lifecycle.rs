//! Exact-prefix installation and removal for the supported local binary profile.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::canonical::{bytes_digest, encode};
use crate::model::Error;
use crate::operations::OperationsConfig;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityManifest {
    pub version: String,
    pub product_version: String,
    pub rust_version: String,
    pub platforms: Vec<String>,
    pub configuration_versions: Vec<String>,
    pub recovery_protocols: Vec<String>,
    pub state_backends: Vec<String>,
    pub activation_backends: Vec<String>,
}

impl Default for CompatibilityManifest {
    fn default() -> Self {
        Self {
            version: "compatibility-v1".into(),
            product_version: env!("CARGO_PKG_VERSION").into(),
            rust_version: "1.97.0".into(),
            platforms: vec!["aarch64-apple-darwin".into(), "aarch64-linux-gnu".into()],
            configuration_versions: vec![
                "operations-config-v0-migratable".into(),
                "operations-config-v1".into(),
            ],
            recovery_protocols: vec!["fsm-v1".into(), "service-v1".into()],
            state_backends: vec![
                "filesystem-v1".into(),
                "postgresql-18-local".into(),
                "s3-compatible-https".into(),
                "redis-stream-8-local".into(),
            ],
            activation_backends: vec![
                "docker-single-host".into(),
                "kubernetes-networkpolicy".into(),
            ],
        }
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
    let executable_bytes = read_regular(&executable, 128 * 1024 * 1024)?;
    let compatibility = encode(&CompatibilityManifest::default())?;
    let config = encode(&OperationsConfig::default())?;
    let payloads = [
        ("bin/anasemble", executable_bytes.as_slice(), 0o755),
        (
            "share/compatibility-v1.json",
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
        &["compatibility-v1.json", "operations-config-v1.json"],
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
        let bytes = read_regular(&path, 128 * 1024 * 1024)?;
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
