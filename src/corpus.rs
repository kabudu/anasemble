use std::fs;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::model::Error;
use crate::protocol::{RecoveryResult, run};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusManifest {
    version: String,
    workspaces: Vec<String>,
}

#[derive(Serialize)]
pub struct CorpusResult {
    pub version: &'static str,
    pub results: Vec<CorpusEntry>,
}

#[derive(Serialize)]
pub struct CorpusEntry {
    pub workspace: String,
    pub result: RecoveryResult,
}

pub fn run_corpus(root: &Path) -> Result<CorpusResult, Error> {
    let data = fs::read(root.join("corpus.json"))?;
    if data.len() > 65_536 {
        return Err(Error::InvalidRegistry(
            "corpus manifest is too large".into(),
        ));
    }
    let manifest: CorpusManifest = serde_json::from_slice(&data)
        .map_err(|error| Error::InvalidRegistry(format!("invalid corpus manifest: {error}")))?;
    if manifest.version != "corpus-v1"
        || manifest.workspaces.is_empty()
        || manifest.workspaces.len() > 256
    {
        return Err(Error::InvalidRegistry(
            "corpus version or workspace count is invalid".into(),
        ));
    }
    let mut results = Vec::with_capacity(manifest.workspaces.len());
    for workspace in manifest.workspaces {
        let path = Path::new(&workspace);
        if path.components().count() != 1
            || !matches!(path.components().next(), Some(Component::Normal(_)))
        {
            return Err(Error::InvalidRegistry(
                "corpus workspace must be one safe relative path component".into(),
            ));
        }
        let workspace_path = root.join(&workspace);
        let metadata = fs::symlink_metadata(&workspace_path)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(Error::InvalidRegistry(
                "corpus workspace must be a real directory".into(),
            ));
        }
        results.push(CorpusEntry {
            workspace: workspace.clone(),
            result: run(&workspace_path),
        });
    }
    Ok(CorpusResult {
        version: "corpus-v1",
        results,
    })
}
