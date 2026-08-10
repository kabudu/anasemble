use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::canonical::{bytes_digest, encode};
use crate::model::Error;
use crate::protocol::RecoveryResult;

#[derive(Debug, Serialize)]
pub struct LedgerReceipt {
    pub entry_id: String,
    pub path: PathBuf,
    pub replay: bool,
}

#[derive(Serialize)]
struct LedgerManifest {
    version: &'static str,
    entry_id: String,
    outcome_digest: String,
    inputs: Vec<InputDigest>,
}

#[derive(Serialize)]
struct InputDigest {
    path: String,
    sha256: String,
    bytes: usize,
}

pub fn persist(
    workspace: &Path,
    ledger_root: &Path,
    result: &RecoveryResult,
) -> Result<LedgerReceipt, Error> {
    fs::create_dir_all(ledger_root)?;
    let outcome = encode(result)?;
    let mut inputs = input_files(workspace)?;
    inputs.sort_by(|left, right| left.0.cmp(&right.0));
    let mut identity = Sha256::new();
    identity.update(b"anasemble-ledger-v1\0");
    identity.update(&outcome);
    let mut input_digests = Vec::with_capacity(inputs.len());
    for (relative, data) in &inputs {
        identity.update(relative.as_bytes());
        identity.update([0]);
        identity.update(data);
        input_digests.push(InputDigest {
            path: relative.clone(),
            sha256: bytes_digest(data),
            bytes: data.len(),
        });
    }
    let entry_id = hex::encode(identity.finalize());
    let target = ledger_root.join(&entry_id);
    if target.try_exists()? {
        verify_existing(&target, &outcome)?;
        return Ok(LedgerReceipt {
            entry_id,
            path: target,
            replay: true,
        });
    }
    let lock_path = ledger_root.join(format!(".{entry_id}.lock"));
    let lock = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|error| Error::Io(std::io::Error::new(error.kind(), "ledger entry is locked")))?;
    let _guard = LockGuard {
        path: lock_path,
        _file: lock,
    };
    if target.try_exists()? {
        verify_existing(&target, &outcome)?;
        return Ok(LedgerReceipt {
            entry_id,
            path: target,
            replay: true,
        });
    }
    let temporary = ledger_root.join(format!(".{entry_id}.tmp"));
    fs::create_dir(&temporary)?;
    let write_result = (|| {
        write_new(&temporary.join("outcome.json"), &outcome)?;
        let input_root = temporary.join("inputs");
        for (relative, data) in &inputs {
            let path = input_root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            write_new(&path, data)?;
        }
        let manifest = LedgerManifest {
            version: "ledger-v1",
            entry_id: entry_id.clone(),
            outcome_digest: bytes_digest(&outcome),
            inputs: input_digests,
        };
        write_new(&temporary.join("manifest.json"), &encode(&manifest)?)?;
        File::open(&temporary)?.sync_all()?;
        fs::rename(&temporary, &target)?;
        File::open(ledger_root)?.sync_all()?;
        Ok::<(), Error>(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
    }
    write_result?;
    Ok(LedgerReceipt {
        entry_id,
        path: target,
        replay: false,
    })
}

fn input_files(workspace: &Path) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let registry_path = workspace.join("registry.json");
    let registry_metadata = fs::symlink_metadata(&registry_path)?;
    if !registry_metadata.is_file()
        || registry_metadata.file_type().is_symlink()
        || registry_metadata.len() > 131_072
    {
        return Err(Error::InvalidEvidence(
            "ledger registry snapshot is not a bounded regular file".into(),
        ));
    }
    let mut files = vec![("registry.json".into(), fs::read(registry_path)?)];
    let mut count = 0_usize;
    for entry in fs::read_dir(workspace.join("fragments"))? {
        count += 1;
        if count > 10_000 {
            return Err(Error::SearchExhausted(
                "ledger fragment snapshot bound exceeded".into(),
            ));
        }
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 1_048_576 {
            return Err(Error::InvalidEvidence(
                "ledger input is not a bounded regular file".into(),
            ));
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::InvalidEvidence("fragment filename is not valid UTF-8".into()))?;
        files.push((format!("fragments/{name}"), fs::read(entry.path())?));
    }
    Ok(files)
}

fn write_new(path: &Path, data: &[u8]) -> Result<(), Error> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(())
}

fn verify_existing(target: &Path, outcome: &[u8]) -> Result<(), Error> {
    if fs::read(target.join("outcome.json"))? != outcome {
        return Err(Error::InvalidEvidence(
            "ledger entry identity collides with different content".into(),
        ));
    }
    Ok(())
}

struct LockGuard {
    path: PathBuf,
    _file: File,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
