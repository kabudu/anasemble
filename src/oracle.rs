use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::model::Error;

#[derive(Debug, Serialize)]
pub struct LossAttestation {
    pub method: &'static str,
    pub recovery_root: PathBuf,
    pub forbidden_paths_absent: bool,
    pub forbidden_digests_absent: bool,
    pub scanned_files: u64,
    pub scanned_bytes: u64,
}

pub fn attest_absence(
    recovery_root: &Path,
    forbidden_paths: &[PathBuf],
    forbidden_digests: &[String],
    max_files: u64,
    max_bytes: u64,
) -> Result<LossAttestation, Error> {
    let root = recovery_root.canonicalize()?;
    for path in forbidden_paths {
        if path.try_exists()? {
            return Err(Error::ArtifactPresent(
                "a declared lost-artifact path still exists".into(),
            ));
        }
    }
    let mut paths = Vec::new();
    let mut visited_entries = 0_u64;
    collect_regular_files(&root, &mut paths, &mut visited_entries, max_files)?;
    paths.sort();
    let mut scanned_files = 0_u64;
    let mut scanned_bytes = 0_u64;
    let mut buffer = [0_u8; 65_536];
    for path in paths {
        scanned_files += 1;
        if scanned_files > max_files {
            return Err(Error::SearchExhausted(
                "workspace file-count bound exceeded".into(),
            ));
        }
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            scanned_bytes = scanned_bytes
                .checked_add(read as u64)
                .ok_or_else(|| Error::SearchExhausted("workspace byte counter overflow".into()))?;
            if scanned_bytes > max_bytes {
                return Err(Error::SearchExhausted(
                    "workspace byte bound exceeded".into(),
                ));
            }
            hasher.update(&buffer[..read]);
        }
        if forbidden_digests.contains(&hex::encode(hasher.finalize())) {
            return Err(Error::ArtifactPresent(
                "lost artifact digest exists in recovery workspace".into(),
            ));
        }
    }
    Ok(LossAttestation {
        method: "path-and-sha256-scan-v1",
        recovery_root: root,
        forbidden_paths_absent: true,
        forbidden_digests_absent: true,
        scanned_files,
        scanned_bytes,
    })
}

fn collect_regular_files(
    directory: &Path,
    output: &mut Vec<PathBuf>,
    visited_entries: &mut u64,
    max_entries: u64,
) -> Result<(), Error> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)? {
        *visited_entries = visited_entries
            .checked_add(1)
            .ok_or_else(|| Error::SearchExhausted("workspace entry counter overflow".into()))?;
        if *visited_entries > max_entries {
            return Err(Error::SearchExhausted(
                "workspace entry-count bound exceeded".into(),
            ));
        }
        entries.push(entry?);
    }
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::InvalidEvidence(
                "recovery workspace contains a symlink".into(),
            ));
        }
        if metadata.is_dir() {
            collect_regular_files(&path, output, visited_entries, max_entries)?;
        } else if metadata.is_file() {
            output.push(path);
        } else {
            return Err(Error::InvalidEvidence(
                "recovery workspace contains a non-regular file".into(),
            ));
        }
    }
    Ok(())
}
