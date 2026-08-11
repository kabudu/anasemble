use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::canonical::{bytes_digest, encode};
use crate::model::Error;

pub const MAX_STATE_BYTES: u64 = 67_108_864;
const MAX_MANIFEST_BYTES: u64 = 16_384;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StateManifest {
    pub version: String,
    pub component: String,
    pub schema_version: String,
    pub revision: u64,
    pub payload_sha256: String,
    pub payload_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct SnapshotReceipt {
    pub manifest_sha256: String,
    pub payload_sha256: String,
    pub payload_bytes: u64,
    pub revision: u64,
}

#[derive(Debug, Serialize)]
pub struct RestoreReceipt {
    pub payload_sha256: String,
    pub payload_bytes: u64,
    pub revision: u64,
    pub rollback_available: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ActivationMarker {
    version: String,
    payload_sha256: String,
    payload_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreFailurePoint {
    None,
    AfterRollbackPrepared,
}

pub fn snapshot(
    store: &Path,
    source: &Path,
    component: &str,
    schema_version: &str,
    revision: u64,
) -> Result<SnapshotReceipt, Error> {
    validate_label("component", component)?;
    validate_label("schema version", schema_version)?;
    let payload = read_regular_bounded(source, MAX_STATE_BYTES, "state source")?;
    let payload_sha256 = bytes_digest(&payload);
    ensure_real_directory(store)?;
    let _lock = Lock::acquire(&store.join(".snapshot.lock"), "state store is locked")?;
    let current = store.join("current.json");
    if current.try_exists()? {
        let current_bytes = read_regular_bounded(&current, MAX_MANIFEST_BYTES, "state manifest")?;
        let current_manifest: StateManifest = serde_json::from_slice(&current_bytes)
            .map_err(|error| invalid(&format!("state manifest is invalid: {error}")))?;
        validate_manifest(&current_manifest)?;
        if current_manifest.component != component || revision <= current_manifest.revision {
            return Err(invalid(
                "state snapshots must retain component identity and increase revision",
            ));
        }
    }
    let objects = store.join("objects");
    ensure_real_directory(&objects)?;
    let object = objects.join(format!("{payload_sha256}.bin"));
    if object.try_exists()? {
        let existing = read_regular_bounded(&object, MAX_STATE_BYTES, "state object")?;
        if existing != payload {
            return Err(invalid(
                "content-addressed state object has conflicting bytes",
            ));
        }
    } else {
        write_new_synced(&object, &payload)?;
        File::open(&objects)?.sync_all()?;
    }
    let manifest = StateManifest {
        version: "state-file-v1".into(),
        component: component.into(),
        schema_version: schema_version.into(),
        revision,
        payload_sha256: payload_sha256.clone(),
        payload_bytes: payload.len() as u64,
    };
    let manifest_bytes = encode(&manifest)?;
    atomic_replace(store, &current, "current.tmp", &manifest_bytes)?;
    Ok(SnapshotReceipt {
        manifest_sha256: bytes_digest(&manifest_bytes),
        payload_sha256,
        payload_bytes: payload.len() as u64,
        revision,
    })
}

pub fn restore(store: &Path, destination: &Path) -> Result<RestoreReceipt, Error> {
    restore_with_failure(store, destination, RestoreFailurePoint::None)
}

pub fn restore_with_failure(
    store: &Path,
    destination: &Path,
    failure: RestoreFailurePoint,
) -> Result<RestoreReceipt, Error> {
    ensure_existing_real_directory(store)?;
    let _store_lock = Lock::acquire(&store.join(".snapshot.lock"), "state store is locked")?;
    let manifest_bytes = read_regular_bounded(
        &store.join("current.json"),
        MAX_MANIFEST_BYTES,
        "state manifest",
    )?;
    let manifest: StateManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| invalid(&format!("state manifest is invalid: {error}")))?;
    validate_manifest(&manifest)?;
    let object = store
        .join("objects")
        .join(format!("{}.bin", manifest.payload_sha256));
    let payload = read_regular_bounded(&object, MAX_STATE_BYTES, "state object")?;
    if payload.len() as u64 != manifest.payload_bytes
        || bytes_digest(&payload) != manifest.payload_sha256
    {
        return Err(invalid(
            "state object integrity does not match its manifest",
        ));
    }

    let parent = destination
        .parent()
        .ok_or_else(|| invalid("state destination requires a parent directory"))?;
    ensure_existing_real_directory(parent)?;
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("state destination requires a UTF-8 file name"))?;
    let lock_path = parent.join(format!(".{name}.anasemble.lock"));
    let _destination_lock = Lock::acquire(&lock_path, "state destination is locked")?;
    let staged = parent.join(format!(".{name}.anasemble-stage"));
    let rollback = rollback_path(destination)?;
    let activation = activation_path(destination)?;
    if staged.try_exists()? || rollback.try_exists()? || activation.try_exists()? {
        return Err(invalid(
            "stale state staging, rollback, or activation file requires operator action",
        ));
    }
    write_new_synced(&staged, &payload)?;
    let had_destination = destination.try_exists()?;
    if had_destination {
        let metadata = fs::symlink_metadata(destination)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            fs::remove_file(&staged)?;
            return Err(invalid(
                "state destination must be a regular non-symlink file",
            ));
        }
        fs::rename(destination, &rollback)?;
        File::open(parent)?.sync_all()?;
    }
    if failure == RestoreFailurePoint::AfterRollbackPrepared {
        fs::remove_file(&staged)?;
        if had_destination {
            fs::rename(&rollback, destination)?;
            File::open(parent)?.sync_all()?;
        }
        return Err(Error::Io(std::io::Error::other(
            "injected failure after state rollback preparation",
        )));
    }
    if let Err(error) = fs::rename(&staged, destination) {
        let _ = fs::remove_file(&staged);
        if had_destination {
            fs::rename(&rollback, destination)?;
            File::open(parent)?.sync_all()?;
        }
        return Err(Error::Io(error));
    }
    if had_destination {
        let marker = encode(&ActivationMarker {
            version: "state-activation-v1".into(),
            payload_sha256: manifest.payload_sha256.clone(),
            payload_bytes: manifest.payload_bytes,
        })?;
        if let Err(error) = write_new_synced(&activation, &marker) {
            fs::rename(&rollback, destination)?;
            File::open(parent)?.sync_all()?;
            return Err(error);
        }
    }
    File::open(parent)?.sync_all()?;
    Ok(RestoreReceipt {
        payload_sha256: manifest.payload_sha256,
        payload_bytes: manifest.payload_bytes,
        revision: manifest.revision,
        rollback_available: had_destination,
    })
}

pub fn rollback(destination: &Path) -> Result<(), Error> {
    let parent = destination
        .parent()
        .ok_or_else(|| invalid("state destination requires a parent directory"))?;
    ensure_existing_real_directory(parent)?;
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("state destination requires a UTF-8 file name"))?;
    let _lock = Lock::acquire(
        &parent.join(format!(".{name}.anasemble.lock")),
        "state destination is locked",
    )?;
    let rollback = rollback_path(destination)?;
    let activation = activation_path(destination)?;
    let previous = read_regular_bounded(&rollback, MAX_STATE_BYTES, "state rollback")?;
    let staged = parent.join(format!(".{name}.anasemble-stage"));
    if staged.try_exists()? {
        return Err(invalid("stale state staging file requires operator action"));
    }
    write_new_synced(&staged, &previous)?;
    fs::rename(&staged, destination)?;
    fs::remove_file(rollback)?;
    if activation.try_exists()? {
        fs::remove_file(activation)?;
    }
    File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn commit(destination: &Path) -> Result<(), Error> {
    let parent = destination
        .parent()
        .ok_or_else(|| invalid("state destination requires a parent directory"))?;
    ensure_existing_real_directory(parent)?;
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("state destination requires a UTF-8 file name"))?;
    let _lock = Lock::acquire(
        &parent.join(format!(".{name}.anasemble.lock")),
        "state destination is locked",
    )?;
    let rollback = rollback_path(destination)?;
    let activation = activation_path(destination)?;
    let metadata = fs::symlink_metadata(&rollback)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_STATE_BYTES
    {
        return Err(invalid("state rollback must be a bounded regular file"));
    }
    let marker_bytes =
        read_regular_bounded(&activation, MAX_MANIFEST_BYTES, "state activation marker")?;
    let marker: ActivationMarker = serde_json::from_slice(&marker_bytes)
        .map_err(|error| invalid(&format!("state activation marker is invalid: {error}")))?;
    let active = read_regular_bounded(destination, MAX_STATE_BYTES, "active state")?;
    if marker.version != "state-activation-v1"
        || marker.payload_bytes != active.len() as u64
        || marker.payload_sha256 != bytes_digest(&active)
    {
        return Err(invalid(
            "active state no longer matches the restored payload; rollback cannot be discarded",
        ));
    }
    fs::remove_file(rollback)?;
    fs::remove_file(activation)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn validate_manifest(manifest: &StateManifest) -> Result<(), Error> {
    if manifest.version != "state-file-v1"
        || manifest.payload_bytes > MAX_STATE_BYTES
        || manifest.payload_sha256.len() != 64
        || hex::decode(&manifest.payload_sha256).map_or(true, |bytes| bytes.len() != 32)
    {
        return Err(invalid(
            "state manifest version, size, or digest is invalid",
        ));
    }
    validate_label("component", &manifest.component)?;
    validate_label("schema version", &manifest.schema_version)
}

fn validate_label(label: &str, value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
    {
        return Err(invalid(&format!("{label} is invalid")));
    }
    Ok(())
}

fn rollback_path(destination: &Path) -> Result<PathBuf, Error> {
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("state destination requires a UTF-8 file name"))?;
    Ok(destination.with_file_name(format!("{name}.anasemble-rollback")))
}

fn activation_path(destination: &Path) -> Result<PathBuf, Error> {
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("state destination requires a UTF-8 file name"))?;
    Ok(destination.with_file_name(format!("{name}.anasemble-activation.json")))
}

fn ensure_real_directory(path: &Path) -> Result<(), Error> {
    if !path.try_exists()? {
        fs::create_dir(path)?;
    }
    ensure_existing_real_directory(path)
}

fn ensure_existing_real_directory(path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid("state path must be a real directory"));
    }
    Ok(())
}

fn read_regular_bounded(path: &Path, max_bytes: u64, label: &str) -> Result<Vec<u8>, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > max_bytes {
        return Err(invalid(&format!("{label} must be a bounded regular file")));
    }
    Ok(fs::read(path)?)
}

fn write_new_synced(path: &Path, data: &[u8]) -> Result<(), Error> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(())
}

fn atomic_replace(
    root: &Path,
    target: &Path,
    temporary_name: &str,
    data: &[u8],
) -> Result<(), Error> {
    let temporary = root.join(temporary_name);
    if temporary.try_exists()? {
        return Err(invalid("stale state manifest staging file"));
    }
    write_new_synced(&temporary, data)?;
    fs::rename(&temporary, target)?;
    File::open(root)?.sync_all()?;
    Ok(())
}

fn invalid(message: &str) -> Error {
    Error::InvalidEvidence(message.into())
}

struct Lock {
    path: PathBuf,
    _file: File,
}

impl Lock {
    fn acquire(path: &Path, message: &str) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| Error::Io(std::io::Error::new(error.kind(), message)))?;
        Ok(Self {
            path: path.into(),
            _file: file,
        })
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
