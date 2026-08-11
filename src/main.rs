use std::env;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::ExitCode;

use anasemble::campaign::run_campaign;
use anasemble::corpus::run_corpus;
use anasemble::deployment::{StateSnapshot, StateTransform, deploy, rollback};
use anasemble::evidence_plane::{self, StoreBundle};
use anasemble::fragments::{self, Envelope};
use anasemble::ledger::persist;
use anasemble::lifecycle;
use anasemble::operations::{OperationsConfig, OperationsStore, RunFailurePoint};
use anasemble::protocol::{RecoveryResult, run};
use anasemble::reference;
use anasemble::service::ServiceManifest;
use anasemble::state_store;

fn main() -> ExitCode {
    match execute() {
        Ok(certified) => {
            if certified {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        Err(error) => {
            eprintln!("anasemble: {error}");
            ExitCode::from(1)
        }
    }
}

fn execute() -> Result<bool, Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    let command = arguments.next().ok_or("command is required")?;
    if command == "install" {
        let prefix = PathBuf::from(arguments.next().ok_or("installation prefix is required")?);
        reject_extra(&mut arguments, "install accepts only a new prefix")?;
        write_json_stdout(&lifecycle::install(&prefix)?)?;
        return Ok(true);
    }
    if command == "uninstall" {
        let prefix = PathBuf::from(arguments.next().ok_or("installation prefix is required")?);
        reject_extra(&mut arguments, "uninstall accepts only an installed prefix")?;
        write_json_stdout(&lifecycle::uninstall(&prefix)?)?;
        return Ok(true);
    }
    if command == "init-operations" {
        let root = PathBuf::from(arguments.next().ok_or("operations root is required")?);
        let config_path = PathBuf::from(arguments.next().ok_or("operations config is required")?);
        reject_extra(
            &mut arguments,
            "init-operations accepts root and configuration",
        )?;
        let config = OperationsConfig::migrate(&read_bounded_regular(&config_path)?)?;
        OperationsStore::create(&root, config)?;
        write_json_stdout(&serde_json::json!({"created": true, "version": "operations-store-v1"}))?;
        return Ok(true);
    }
    if command == "migrate-operations-config" {
        let input = PathBuf::from(arguments.next().ok_or("input config is required")?);
        let output = PathBuf::from(arguments.next().ok_or("output config is required")?);
        reject_extra(
            &mut arguments,
            "migrate-operations-config accepts input and output paths",
        )?;
        let config = OperationsConfig::migrate(&read_bounded_regular(&input)?)?;
        write_new_json(&output, &config)?;
        write_json_stdout(&serde_json::json!({
            "migrated": true,
            "version": config.version
        }))?;
        return Ok(true);
    }
    if command == "enqueue-recovery" {
        let root = PathBuf::from(arguments.next().ok_or("operations root is required")?);
        let workspace = PathBuf::from(arguments.next().ok_or("workspace is required")?);
        let submitted_unix = u64_argument(arguments.next(), "submitted unix time")?;
        reject_extra(
            &mut arguments,
            "enqueue-recovery accepts root, workspace, and submitted unix time",
        )?;
        write_json_stdout(&OperationsStore::open(&root)?.enqueue(&workspace, submitted_unix)?)?;
        return Ok(true);
    }
    if command == "run-jobs" {
        let root = PathBuf::from(arguments.next().ok_or("operations root is required")?);
        let now_unix = u64_argument(arguments.next(), "current unix time")?;
        reject_extra(
            &mut arguments,
            "run-jobs accepts root and current unix time",
        )?;
        let receipt =
            OperationsStore::open(&root)?.run_recovery_batch(now_unix, RunFailurePoint::None)?;
        write_json_stdout(&receipt)?;
        return Ok(true);
    }
    if command == "operations-status" {
        let root = PathBuf::from(arguments.next().ok_or("operations root is required")?);
        reject_extra(&mut arguments, "operations-status accepts only a root")?;
        write_json_stdout(&OperationsStore::open(&root)?.status()?)?;
        return Ok(true);
    }
    if command == "job-result" {
        let root = PathBuf::from(arguments.next().ok_or("operations root is required")?);
        let job_id = utf8_argument(arguments.next(), "job id")?;
        reject_extra(&mut arguments, "job-result accepts root and job id")?;
        write_json_stdout(&OperationsStore::open(&root)?.result(&job_id)?)?;
        return Ok(true);
    }
    if command == "create-support-bundle" {
        let root = PathBuf::from(arguments.next().ok_or("operations root is required")?);
        let generated_unix = u64_argument(arguments.next(), "generated unix time")?;
        let output = PathBuf::from(
            arguments
                .next()
                .ok_or("support bundle output is required")?,
        );
        reject_extra(
            &mut arguments,
            "create-support-bundle accepts root, generated unix time, and output",
        )?;
        let bundle = OperationsStore::open(&root)?.support_bundle(generated_unix)?;
        write_new_private_json(&output, &bundle)?;
        write_json_stdout(&serde_json::json!({"bundle_sha256": bundle.bundle_sha256}))?;
        return Ok(true);
    }
    if command == "prune-jobs" {
        let root = PathBuf::from(arguments.next().ok_or("operations root is required")?);
        let submitted_before = u64_argument(arguments.next(), "submitted-before unix time")?;
        let max_remove = usize::try_from(u64_argument(arguments.next(), "maximum removals")?)?;
        reject_extra(
            &mut arguments,
            "prune-jobs accepts root, submitted-before unix time, and maximum removals",
        )?;
        write_json_stdout(
            &OperationsStore::open(&root)?.prune_terminal(submitted_before, max_remove)?,
        )?;
        return Ok(true);
    }
    if command == "prepare-reference-recovery" {
        let config_path = PathBuf::from(arguments.next().ok_or("reference config is required")?);
        let output = PathBuf::from(arguments.next().ok_or("state bundle output is required")?);
        reject_extra(
            &mut arguments,
            "prepare-reference-recovery accepts config and a new bundle path",
        )?;
        let config = reference::read_config(&config_path)?;
        let bundle = reference::prepare(&config)?;
        write_new_private_json(&output, &bundle)?;
        write_json_stdout(&serde_json::json!({
            "version": bundle.version,
            "bundle_sha256": bundle.bundle_sha256
        }))?;
        return Ok(true);
    }
    if command == "recover-activate-reference" {
        let config_path = PathBuf::from(arguments.next().ok_or("reference config is required")?);
        let bundle_path = PathBuf::from(arguments.next().ok_or("state bundle is required")?);
        let output = PathBuf::from(
            arguments
                .next()
                .ok_or("recovery receipt output is required")?,
        );
        reject_extra(
            &mut arguments,
            "recover-activate-reference accepts config, bundle, and a new receipt path",
        )?;
        let config = reference::read_config(&config_path)?;
        let bundle = reference::read_bundle(&bundle_path)?;
        let receipt = reference::recover_and_activate(&config, &bundle)?;
        write_new_private_json(&output, &receipt)?;
        write_json_stdout(&serde_json::json!({
            "version": receipt.version,
            "plan_sha256": receipt.activation_plan.plan_sha256,
            "immutable_image": receipt.artifact.immutable_image,
            "service": receipt.activation.service,
            "rollback_available": receipt.activation.rollback_available
        }))?;
        return Ok(true);
    }
    if command == "rollback-reference-recovery" {
        let config_path = PathBuf::from(arguments.next().ok_or("reference config is required")?);
        let receipt_path = PathBuf::from(arguments.next().ok_or("recovery receipt is required")?);
        reject_extra(
            &mut arguments,
            "rollback-reference-recovery accepts config and receipt",
        )?;
        let config = reference::read_config(&config_path)?;
        let receipt = reference::read_receipt(&receipt_path)?;
        reference::rollback_recovery(&config, &receipt)?;
        write_json_stdout(&serde_json::json!({
            "rolled_back": true,
            "plan_sha256": receipt.activation_plan.plan_sha256
        }))?;
        return Ok(true);
    }
    if command == "create-signing-key" {
        let path = PathBuf::from(arguments.next().ok_or("signing key path is required")?);
        let key_id = utf8_argument(arguments.next(), "key id")?;
        let created_at = utf8_argument(arguments.next(), "created_at")?;
        reject_extra(
            &mut arguments,
            "create-signing-key accepts path, key id, and created_at",
        )?;
        fragments::create_signing_key(&path, &key_id, &created_at)?;
        write_json_stdout(&serde_json::json!({"created": true, "key_id": key_id}))?;
        return Ok(true);
    }
    if command == "sign-fragment" {
        let input = PathBuf::from(arguments.next().ok_or("fragment path is required")?);
        let key_path = PathBuf::from(arguments.next().ok_or("signing key path is required")?);
        let output = PathBuf::from(
            arguments
                .next()
                .ok_or("signed fragment output is required")?,
        );
        reject_extra(
            &mut arguments,
            "sign-fragment accepts input, key, and output",
        )?;
        let envelope: Envelope = serde_json::from_slice(&read_bounded_regular(&input)?)?;
        let key = fragments::read_signing_key(&key_path)?;
        write_new_json(&output, &fragments::sign_with_key_file(envelope, &key)?)?;
        return Ok(true);
    }
    if command == "create-recovery-key" {
        let path = PathBuf::from(arguments.next().ok_or("recovery key path is required")?);
        let key_id = utf8_argument(arguments.next(), "key id")?;
        let created_at = utf8_argument(arguments.next(), "created_at")?;
        reject_extra(
            &mut arguments,
            "create-recovery-key accepts path, key id, and created_at",
        )?;
        evidence_plane::create_recovery_key(&path, &key_id, &created_at)?;
        write_json_stdout(&serde_json::json!({"created": true, "key_id": key_id}))?;
        return Ok(true);
    }
    if command == "seal-evidence" {
        let input = PathBuf::from(arguments.next().ok_or("signed fragment path is required")?);
        let key_path = PathBuf::from(arguments.next().ok_or("recovery key path is required")?);
        let created_at = utf8_argument(arguments.next(), "created_at")?;
        let delete_after = utf8_argument(arguments.next(), "delete_after")?;
        let output = PathBuf::from(
            arguments
                .next()
                .ok_or("sealed evidence output is required")?,
        );
        reject_extra(
            &mut arguments,
            "seal-evidence accepts fragment, key, created_at, delete_after, and output",
        )?;
        let envelope: Envelope = serde_json::from_slice(&read_bounded_regular(&input)?)?;
        let key = evidence_plane::read_recovery_key(&key_path)?;
        write_new_json(
            &output,
            &evidence_plane::seal(&envelope, &key, &created_at, &delete_after)?,
        )?;
        return Ok(true);
    }
    if command == "sign-store-bundle" {
        let input = PathBuf::from(arguments.next().ok_or("store bundle path is required")?);
        let key_path = PathBuf::from(
            arguments
                .next()
                .ok_or("store signing key path is required")?,
        );
        let output = PathBuf::from(arguments.next().ok_or("signed bundle output is required")?);
        reject_extra(
            &mut arguments,
            "sign-store-bundle accepts bundle, key, and output",
        )?;
        let bundle: StoreBundle = serde_json::from_slice(&read_bounded_bundle(&input)?)?;
        let key = fragments::read_signing_key(&key_path)?;
        write_new_json(
            &output,
            &evidence_plane::sign_bundle_with_key_file(bundle, &key)?,
        )?;
        return Ok(true);
    }
    if command == "retrieve-evidence" {
        let config = PathBuf::from(arguments.next().ok_or("evidence config path is required")?);
        let output = PathBuf::from(arguments.next().ok_or("evidence output path is required")?);
        reject_extra(
            &mut arguments,
            "retrieve-evidence accepts config and output directory",
        )?;
        let receipt = evidence_plane::materialize(&config, &output)?;
        write_json_stdout(&receipt)?;
        return Ok(true);
    }
    if command == "delete-evidence" {
        let output = PathBuf::from(arguments.next().ok_or("evidence output path is required")?);
        reject_extra(
            &mut arguments,
            "delete-evidence accepts only an output directory",
        )?;
        let removed = evidence_plane::delete_materialized(&output)?;
        write_json_stdout(&serde_json::json!({"removed_files": removed}))?;
        return Ok(true);
    }
    if command == "delete-store-bundle" {
        let bundle = PathBuf::from(arguments.next().ok_or("store bundle path is required")?);
        reject_extra(
            &mut arguments,
            "delete-store-bundle accepts only a bundle path",
        )?;
        let digest = evidence_plane::delete_store_bundle(&bundle)?;
        write_json_stdout(&serde_json::json!({"deleted_bundle_sha256": digest}))?;
        return Ok(true);
    }
    if command == "validate-service" {
        let path = PathBuf::from(
            arguments
                .next()
                .ok_or("service manifest path is required")?,
        );
        if arguments.next().is_some() {
            return Err("validate-service accepts only a manifest path".into());
        }
        let manifest: ServiceManifest = serde_json::from_slice(&read_bounded_regular(&path)?)?;
        manifest.validate()?;
        let receipt = serde_json::json!({
            "version": manifest.version,
            "component": manifest.component,
            "interface_version": manifest.interface_version,
            "manifest_sha256": anasemble::canonical::digest(&manifest)?,
            "endpoint_count": manifest.http.endpoints.len(),
            "state_dependency_count": manifest.state_dependencies.len()
        });
        let mut encoded = serde_json::to_vec(&receipt)?;
        encoded.push(b'\n');
        io::stdout().write_all(&encoded)?;
        return Ok(true);
    }
    if command == "snapshot-state" {
        let store = PathBuf::from(arguments.next().ok_or("state store path is required")?);
        let source = PathBuf::from(arguments.next().ok_or("state source path is required")?);
        let component = arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or("component is required and must be UTF-8")?;
        let schema = arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or("schema version is required and must be UTF-8")?;
        let revision: u64 = arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or("revision is required and must be UTF-8")?
            .parse()?;
        if arguments.next().is_some() {
            return Err(
                "snapshot-state accepts store, source, component, schema, and revision".into(),
            );
        }
        let receipt = state_store::snapshot(&store, &source, &component, &schema, revision)?;
        write_json_stdout(&receipt)?;
        return Ok(true);
    }
    if command == "restore-state" {
        let store = PathBuf::from(arguments.next().ok_or("state store path is required")?);
        let destination = PathBuf::from(
            arguments
                .next()
                .ok_or("state destination path is required")?,
        );
        if arguments.next().is_some() {
            return Err("restore-state accepts only store and destination".into());
        }
        let receipt = state_store::restore(&store, &destination)?;
        write_json_stdout(&receipt)?;
        return Ok(true);
    }
    if command == "rollback-state" {
        let destination = PathBuf::from(
            arguments
                .next()
                .ok_or("state destination path is required")?,
        );
        if arguments.next().is_some() {
            return Err("rollback-state accepts only a destination".into());
        }
        state_store::rollback(&destination)?;
        write_json_stdout(&serde_json::json!({"rolled_back": true}))?;
        return Ok(true);
    }
    if command == "commit-state" {
        let destination = PathBuf::from(
            arguments
                .next()
                .ok_or("state destination path is required")?,
        );
        if arguments.next().is_some() {
            return Err("commit-state accepts only a destination".into());
        }
        state_store::commit(&destination)?;
        write_json_stdout(&serde_json::json!({"committed": true}))?;
        return Ok(true);
    }
    if command == "evaluate-campaign" {
        let root = PathBuf::from(arguments.next().ok_or("campaign root is required")?);
        if arguments.next().is_some() {
            return Err("evaluate-campaign accepts only a campaign root".into());
        }
        let report = run_campaign(&root)?;
        let successful = report.metrics.unsafe_certifications == 0
            && report.cases.iter().all(|case| case.matched_expectation);
        let mut encoded = serde_json::to_vec(&report)?;
        encoded.push(b'\n');
        io::stdout().write_all(&encoded)?;
        return Ok(successful);
    }
    if command == "deploy" {
        let workspace = PathBuf::from(arguments.next().ok_or("workspace path is required")?);
        let state_path = PathBuf::from(arguments.next().ok_or("state path is required")?);
        let transform_path =
            PathBuf::from(arguments.next().ok_or("state transform path is required")?);
        let deployment_root = PathBuf::from(arguments.next().ok_or("deployment root is required")?);
        if arguments.next().is_some() {
            return Err("deploy accepts workspace, state, transform, and deployment root".into());
        }
        let state: StateSnapshot = serde_json::from_slice(&read_bounded_regular(&state_path)?)?;
        let transform: StateTransform =
            serde_json::from_slice(&read_bounded_regular(&transform_path)?)?;
        let result = run(&workspace);
        let receipt = deploy(&deployment_root, &result, &state, &transform)?;
        let mut encoded = serde_json::to_vec(&receipt)?;
        encoded.push(b'\n');
        io::stdout().write_all(&encoded)?;
        return Ok(true);
    }
    if command == "rollback" {
        let deployment_root = PathBuf::from(arguments.next().ok_or("deployment root is required")?);
        if arguments.next().is_some() {
            return Err("rollback accepts only a deployment root".into());
        }
        let receipt = rollback(&deployment_root)?;
        let mut encoded = serde_json::to_vec(&receipt)?;
        encoded.push(b'\n');
        io::stdout().write_all(&encoded)?;
        return Ok(true);
    }
    if command == "recover-corpus" {
        let root = PathBuf::from(arguments.next().ok_or("corpus root is required")?);
        if arguments.next().is_some() {
            return Err("recover-corpus accepts only a corpus root".into());
        }
        let result = run_corpus(&root)?;
        let mut encoded = serde_json::to_vec(&result)?;
        encoded.push(b'\n');
        io::stdout().write_all(&encoded)?;
        return Ok(result
            .results
            .iter()
            .all(|entry| entry.result.is_certified()));
    }
    if command != "recover" {
        return Err("usage: anasemble <install|uninstall|init-operations|migrate-operations-config|enqueue-recovery|run-jobs|operations-status|job-result|create-support-bundle|prune-jobs|prepare-reference-recovery|recover-activate-reference|rollback-reference-recovery|create-signing-key|sign-fragment|create-recovery-key|seal-evidence|sign-store-bundle|retrieve-evidence|delete-evidence|delete-store-bundle|validate-service|snapshot-state|restore-state|rollback-state|commit-state|recover|recover-corpus|evaluate-campaign|deploy|rollback> ...".into());
    }
    let workspace = PathBuf::from(arguments.next().ok_or("workspace path is required")?);
    let mut output = None;
    let mut ledger = None;
    while let Some(flag) = arguments.next() {
        match flag.to_str() {
            Some("--output") => {
                output = Some(PathBuf::from(
                    arguments.next().ok_or("--output requires a path")?,
                ))
            }
            Some("--ledger") => {
                ledger = Some(PathBuf::from(
                    arguments.next().ok_or("--ledger requires a path")?,
                ))
            }
            _ => return Err("only --output and --ledger may follow the workspace".into()),
        }
    }
    if arguments.next().is_some() {
        return Err("unexpected extra arguments".into());
    }
    let result = run(&workspace);
    if let Some(root) = ledger {
        persist(&workspace, &root, &result)?;
    }
    let mut encoded = serde_json::to_vec(&result)?;
    encoded.push(b'\n');
    if let Some(path) = output {
        fs::write(path, encoded)?;
    } else {
        io::stdout().write_all(&encoded)?;
    }
    Ok(matches!(result, RecoveryResult::Certified { .. }))
}

fn write_json_stdout<T: serde::Serialize>(value: &T) -> Result<(), Box<dyn std::error::Error>> {
    let mut encoded = serde_json::to_vec(value)?;
    encoded.push(b'\n');
    io::stdout().write_all(&encoded)?;
    Ok(())
}

fn write_new_json<T: serde::Serialize>(
    path: &std::path::Path,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let mut bytes = anasemble::canonical::encode(value)?;
    bytes.push(b'\n');
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn write_new_private_json<T: serde::Serialize>(
    path: &std::path::Path,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    let mut bytes = anasemble::canonical::encode(value)?;
    bytes.push(b'\n');
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn utf8_argument(
    argument: Option<std::ffi::OsString>,
    label: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    argument
        .and_then(|value| value.into_string().ok())
        .ok_or_else(|| format!("{label} is required and must be UTF-8").into())
}

fn u64_argument(
    argument: Option<std::ffi::OsString>,
    label: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(utf8_argument(argument, label)?.parse()?)
}

fn reject_extra(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
    usage: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if arguments.next().is_some() {
        return Err(usage.into());
    }
    Ok(())
}

fn read_bounded_regular(path: &std::path::Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 65_536 {
        return Err("state input must be a regular file no larger than 64 KiB".into());
    }
    Ok(fs::read(path)?)
}

fn read_bounded_bundle(path: &std::path::Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 16_777_216 {
        return Err("bundle input must be a regular file no larger than 16 MiB".into());
    }
    Ok(fs::read(path)?)
}
