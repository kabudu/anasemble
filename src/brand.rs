use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const ROOT: &str = "assets/brand";
const SOURCES: &[&str] = &[
    "anasemble-architecture.svg",
    "anasemble-horizontal.svg",
    "anasemble-readme-hero-narrow.svg",
    "anasemble-readme-hero.svg",
    "anasemble-result-icons.svg",
    "anasemble-small.svg",
    "anasemble-stacked.svg",
    "anasemble-symbol-mono.svg",
    "anasemble-symbol-reversed.svg",
    "anasemble-symbol.svg",
    "anasemble-wordmark.svg",
];

#[derive(Debug, Deserialize, Serialize)]
struct Manifest {
    schema: String,
    brand_version: String,
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Entry {
    path: String,
    sha256: String,
    source_sha256: String,
    dimensions: String,
    colour_space: String,
    licence: String,
    provenance: String,
    allowed_use: String,
    export_command: String,
}

pub fn generate(root: &Path) -> Result<(), String> {
    let brand = root.join(ROOT);
    fs::create_dir_all(brand.join("exports"))
        .map_err(|error| format!("could not create export directory: {error}"))?;
    for name in SOURCES {
        let source = brand.join("source").join(name);
        let export = brand.join("exports").join(name);
        fs::copy(&source, &export)
            .map_err(|error| format!("could not export {}: {error}", source.display()))?;
    }
    for (source_name, legacy_name) in [
        ("anasemble-symbol.svg", "assets/anasemble-mark.svg"),
        ("anasemble-horizontal.svg", "assets/anasemble-wordmark.svg"),
    ] {
        fs::copy(
            brand.join("source").join(source_name),
            root.join(legacy_name),
        )
        .map_err(|error| format!("could not update compatibility asset {legacy_name}: {error}"))?;
    }
    let paths = inventory(&brand)?;
    let mut entries = Vec::with_capacity(paths.len());
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "brand path escaped repository root".to_owned())?;
        let bytes = fs::read(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let source_sha256 = if relative.starts_with("assets/brand/exports") {
            let source = brand.join("source").join(
                path.file_name()
                    .ok_or_else(|| "export has no file name".to_owned())?,
            );
            digest(&fs::read(&source).map_err(|error| {
                format!("could not read export source {}: {error}", source.display())
            })?)
        } else {
            digest(&bytes)
        };
        entries.push(Entry {
            path: relative.to_string_lossy().into_owned(),
            sha256: digest(&bytes),
            source_sha256,
            dimensions: if path.extension().is_some_and(|ext| ext == "svg") {
                "scalable SVG viewBox".into()
            } else {
                "not applicable".into()
            },
            colour_space: "sRGB tokens; monochrome-safe".into(),
            licence: "Apache-2.0; trademark rights excluded".into(),
            provenance: "owner-selected Semantic Fit; project-authored vector or text source"
                .into(),
            allowed_use: "private product, documentation, repository and release preparation"
                .into(),
            export_command: "cargo run --locked --offline --bin brand_assets -- generate".into(),
        });
    }
    let manifest = Manifest {
        schema: "anasemble-brand-asset-manifest-v1".into(),
        brand_version: "1.2.0".into(),
        entries,
    };
    let mut encoded = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("could not encode brand manifest: {error}"))?;
    encoded.push(b'\n');
    fs::write(brand.join("BRAND_ASSET_MANIFEST.json"), encoded)
        .map_err(|error| format!("could not write brand manifest: {error}"))
}

pub fn validate(root: &Path) -> Result<(), String> {
    let brand = root.join(ROOT);
    let manifest_bytes = fs::read(brand.join("BRAND_ASSET_MANIFEST.json"))
        .map_err(|error| format!("could not read brand manifest: {error}"))?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("brand manifest is invalid: {error}"))?;
    if manifest.schema != "anasemble-brand-asset-manifest-v1" || manifest.brand_version != "1.2.0" {
        return Err("brand manifest schema or version is unsupported".into());
    }
    let actual = inventory(&brand)?
        .into_iter()
        .map(|path| {
            path.strip_prefix(root)
                .map(|relative| relative.to_string_lossy().into_owned())
                .map_err(|_| "brand path escaped repository root".to_owned())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let declared = manifest
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    if actual != declared {
        return Err("brand manifest inventory does not match assets".into());
    }
    for entry in &manifest.entries {
        let path = root.join(&entry.path);
        let bytes = fs::read(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        if digest(&bytes) != entry.sha256 {
            return Err(format!("brand digest mismatch: {}", entry.path));
        }
        if path.extension().is_some_and(|ext| ext == "svg") {
            validate_svg(&entry.path, &bytes)?;
        }
        if entry.path.contains("/exports/") && entry.sha256 != entry.source_sha256 {
            return Err(format!(
                "export is not reproducible from source: {}",
                entry.path
            ));
        }
    }
    validate_tokens(&brand.join("tokens/brand-tokens.json"))?;
    validate_semantic_redundancy(&brand)?;
    Ok(())
}

fn inventory(brand: &Path) -> Result<Vec<PathBuf>, String> {
    fn visit(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
        for entry in fs::read_dir(directory)
            .map_err(|error| format!("could not read {}: {error}", directory.display()))?
        {
            let path = entry
                .map_err(|error| format!("invalid directory entry: {error}"))?
                .path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'))
            {
                return Err(format!(
                    "hidden file is forbidden in brand assets: {}",
                    path.display()
                ));
            }
            if path.is_dir() {
                visit(&path, paths)?;
            } else if path
                .file_name()
                .is_none_or(|name| name != "BRAND_ASSET_MANIFEST.json")
            {
                paths.push(path);
            }
        }
        Ok(())
    }
    let mut paths = Vec::new();
    visit(brand, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn validate_svg(name: &str, bytes: &[u8]) -> Result<(), String> {
    let text = std::str::from_utf8(bytes).map_err(|_| format!("SVG is not UTF-8: {name}"))?;
    for forbidden in [
        "<script",
        "<foreignobject",
        "javascript:",
        "href=\"http",
        "href='http",
        "xlink:href",
        "data:image",
        " onload=",
        " onclick=",
    ] {
        if text.to_ascii_lowercase().contains(forbidden) {
            return Err(format!(
                "unsafe or remote SVG content in {name}: {forbidden}"
            ));
        }
    }
    if !text.contains("role=\"img\"") || !text.contains("<title") || !text.contains("<desc") {
        return Err(format!("SVG lacks accessible title/description: {name}"));
    }
    let canonical = name.contains("/source/") || name.contains("/exports/");
    if canonical {
        let lower = text.to_ascii_lowercase();
        for term in [
            "alpha",
            "beta",
            "preview",
            "release candidate",
            "production-ready",
            "experimental",
            "evaluation",
        ] {
            if lower.contains(term) {
                return Err(format!("maturity term in canonical asset {name}: {term}"));
            }
        }
    }
    Ok(())
}

fn validate_tokens(path: &Path) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("could not read tokens: {error}"))?,
    )
    .map_err(|error| format!("tokens are invalid JSON: {error}"))?;
    if value["version"] != "1.2.0" {
        return Err("brand token version must match brand 1.2.0".into());
    }
    for pair in [
        ("#0B1628", "#FFFFFF"),
        ("#334155", "#FFFFFF"),
        ("#FFFFFF", "#2457A6"),
    ] {
        if contrast(pair.0, pair.1)? < 4.5 {
            return Err(format!(
                "brand contrast below WCAG AA: {} on {}",
                pair.0, pair.1
            ));
        }
    }
    if value["space"]["minimum_symbol_px"] != 16 {
        return Err("small-size token must remain 16 px".into());
    }
    Ok(())
}

fn validate_semantic_redundancy(brand: &Path) -> Result<(), String> {
    for relative in [
        "templates/diagram-key.svg",
        "source/anasemble-result-icons.svg",
    ] {
        let text = fs::read_to_string(brand.join(relative))
            .map_err(|error| format!("could not read semantic asset {relative}: {error}"))?;
        for label in ["PASS", "REFUSE", "UNCERTAIN", "EVIDENCE"] {
            if !text.contains(label) {
                return Err(format!(
                    "semantic asset {relative} lacks text label {label}"
                ));
            }
        }
        for shape in ["<circle", "<path", "<rect"] {
            if !text.contains(shape) {
                return Err(format!(
                    "semantic asset {relative} lacks redundant shape {shape}"
                ));
            }
        }
    }
    let chart = fs::read_to_string(brand.join("templates/chart-key.svg"))
        .map_err(|error| format!("could not read chart key: {error}"))?;
    for state in ["PASS", "REFUSE", "UNCERTAIN", "EXCLUDED"] {
        if !chart.contains(state) {
            return Err(format!("chart key drops required state {state}"));
        }
    }
    if !chart.contains("m180 20 32 32m0-32-32 32") || !chart.contains("M380 28h32") {
        return Err("chart key lacks pattern redundancy for colour-vision safety".into());
    }
    Ok(())
}

fn contrast(a: &str, b: &str) -> Result<f64, String> {
    fn luminance(hex: &str) -> Result<f64, String> {
        if hex.len() != 7 || !hex.starts_with('#') {
            return Err("invalid colour token".into());
        }
        let mut channels = [0.0; 3];
        for (index, channel) in channels.iter_mut().enumerate() {
            let start = 1 + index * 2;
            let value = u8::from_str_radix(&hex[start..start + 2], 16)
                .map_err(|_| "invalid colour token".to_owned())? as f64
                / 255.0;
            *channel = if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            };
        }
        Ok(0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2])
    }
    let (la, lb) = (luminance(a)?, luminance(b)?);
    Ok((la.max(lb) + 0.05) / (la.min(lb) + 0.05))
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::contrast;
    #[test]
    fn canonical_contrast_pairs_are_accessible() {
        assert!(contrast("#0B1628", "#FFFFFF").unwrap() > 10.0);
        assert!(contrast("#FFFFFF", "#2457A6").unwrap() >= 4.5);
    }
}
