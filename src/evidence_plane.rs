use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use chacha20poly1305::aead::{Aead, Generate, Key, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::canonical::{digest, encode};
use crate::fragments::{CollectedEvidence, Envelope, IssuerPolicy, collect_at};
use crate::model::Error;

const MAX_CONFIG_BYTES: u64 = 262_144;
const MAX_BUNDLE_BYTES: u64 = 16_777_216;
const MAX_STORES: usize = 32;
const MAX_PARALLEL: usize = 8;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryKeyFile {
    pub version: String,
    pub key_id: String,
    pub key_hex: String,
    pub created_at: String,
}

impl Drop for RecoveryKeyFile {
    fn drop(&mut self) {
        self.key_hex.zeroize();
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SealedEvidence {
    pub version: String,
    pub key_id: String,
    pub nonce_hex: String,
    pub ciphertext_hex: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProtectedEvidence {
    created_at: String,
    delete_after: String,
    envelope: Envelope,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoreBundle {
    pub version: String,
    pub store_id: String,
    pub failure_domain: String,
    pub generation: u64,
    pub evidence: Vec<SealedEvidence>,
    pub signature: String,
}

#[derive(Serialize)]
struct UnsignedBundle<'a> {
    version: &'a str,
    store_id: &'a str,
    failure_domain: &'a str,
    generation: u64,
    evidence: &'a [SealedEvidence],
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePlaneConfig {
    pub component: String,
    pub interface_version: String,
    pub required_fragment_domains: usize,
    pub required_stores: usize,
    pub required_copies: usize,
    pub max_parallel: usize,
    pub timeout_ms: u64,
    pub retry_budget: u32,
    pub verification_time: String,
    pub stores: Vec<StoreConfig>,
    pub trusted_issuers: BTreeMap<String, IssuerPolicy>,
    pub recovery_keys: BTreeMap<String, PathBuf>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreConfig {
    pub store_id: String,
    pub failure_domain: String,
    pub public_key: String,
    pub minimum_generation: u64,
    pub transport: StoreTransport,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StoreTransport {
    LocalDirectory {
        path: PathBuf,
    },
    HttpsBundle {
        url: String,
        #[serde(default)]
        bearer_token_env: Option<String>,
    },
}

#[derive(Debug, Serialize)]
pub struct StoreProvenance {
    pub store_id: String,
    pub failure_domain: String,
    pub generation: u64,
    pub bundle_digest: String,
    pub evidence_count: usize,
}

#[derive(Debug, Serialize)]
pub struct EvidencePlaneReceipt {
    pub successful_stores: usize,
    pub failed_stores: Vec<String>,
    pub provenance: Vec<StoreProvenance>,
    pub fragment_domains: Vec<String>,
    pub envelope_count: usize,
    pub verification_audit: Vec<crate::fragments::VerificationAudit>,
}

pub struct RetrievedEvidence {
    pub receipt: EvidencePlaneReceipt,
    pub envelopes: Vec<Envelope>,
}

pub fn create_recovery_key(path: &Path, key_id: &str, created_at: &str) -> Result<(), Error> {
    validate_id("recovery key id", key_id)?;
    parse_time(created_at, "recovery key created_at")?;
    let key = Key::<XChaCha20Poly1305>::generate();
    let file = RecoveryKeyFile {
        version: "evidence-key-v1".into(),
        key_id: key_id.into(),
        key_hex: hex::encode(key),
        created_at: created_at.into(),
    };
    write_secret_new(path, &encode(&file)?)
}

pub fn read_recovery_key(path: &Path) -> Result<RecoveryKeyFile, Error> {
    let bytes = read_secret(path)?;
    let key: RecoveryKeyFile = serde_json::from_slice(&bytes).map_err(|error| {
        Error::InvalidRegistry(format!("recovery key file is invalid: {error}"))
    })?;
    validate_key_file(&key)?;
    Ok(key)
}

pub fn materialize(config_path: &Path, output: &Path) -> Result<EvidencePlaneReceipt, Error> {
    let retrieved = retrieve(config_path)?;
    if output.try_exists()? {
        return Err(Error::InvalidEvidence(
            "evidence output directory already exists".into(),
        ));
    }
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(output)?;
    let fragment_directory = output.join("fragments");
    if let Err(error) = builder.create(&fragment_directory) {
        let _ = fs::remove_dir(output);
        return Err(Error::Io(error));
    }
    let mut created = Vec::new();
    for (index, envelope) in retrieved.envelopes.iter().enumerate() {
        let path = fragment_directory.join(format!("fragment-{index:05}.json"));
        if let Err(error) = write_regular_new(&path, &encode(envelope)?) {
            for created_path in created {
                let _ = fs::remove_file(created_path);
            }
            let _ = fs::remove_dir(&fragment_directory);
            let _ = fs::remove_dir(output);
            return Err(error);
        }
        created.push(path);
    }
    let receipt_path = output.join("receipt.json");
    if let Err(error) = write_regular_new(&receipt_path, &encode(&retrieved.receipt)?) {
        for created_path in created {
            let _ = fs::remove_file(created_path);
        }
        let _ = fs::remove_dir(&fragment_directory);
        let _ = fs::remove_dir(output);
        return Err(error);
    }
    Ok(retrieved.receipt)
}

pub fn delete_materialized(output: &Path) -> Result<usize, Error> {
    let metadata = fs::symlink_metadata(output)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::InvalidEvidence(
            "evidence output must be a real directory".into(),
        ));
    }
    let receipt = output.join("receipt.json");
    read_regular_bounded(&receipt, MAX_BUNDLE_BYTES, "evidence receipt")?;
    let fragment_directory = output.join("fragments");
    let fragment_metadata = fs::symlink_metadata(&fragment_directory)?;
    if !fragment_metadata.is_dir() || fragment_metadata.file_type().is_symlink() {
        return Err(Error::InvalidEvidence(
            "evidence fragments path must be a real directory".into(),
        ));
    }
    let root_entries = fs::read_dir(output)?.count();
    if root_entries != 2 {
        return Err(Error::InvalidEvidence(
            "evidence output root contains an unexpected entry".into(),
        ));
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(&fragment_directory)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > MAX_BUNDLE_BYTES
            || path.extension().and_then(|value| value.to_str()) != Some("json")
        {
            return Err(Error::InvalidEvidence(
                "evidence output contains an unexpected entry".into(),
            ));
        }
        paths.push(path);
        if paths.len() > 10_000 {
            return Err(Error::SearchExhausted(
                "evidence output entry bound exceeded".into(),
            ));
        }
    }
    for path in &paths {
        fs::remove_file(path)?;
    }
    fs::remove_dir(fragment_directory)?;
    fs::remove_file(receipt)?;
    fs::remove_dir(output)?;
    Ok(paths.len() + 1)
}

pub fn delete_store_bundle(path: &Path) -> Result<String, Error> {
    let bytes = read_regular_bounded(path, MAX_BUNDLE_BYTES, "store bundle")?;
    let _: StoreBundle = serde_json::from_slice(&bytes)
        .map_err(|error| Error::InvalidEvidence(format!("store bundle is invalid: {error}")))?;
    let removed_digest = crate::canonical::bytes_digest(&bytes);
    let parent = path
        .parent()
        .ok_or_else(|| Error::InvalidEvidence("store bundle requires a parent directory".into()))?;
    fs::remove_file(path)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(removed_digest)
}

pub fn seal(
    envelope: &Envelope,
    key: &RecoveryKeyFile,
    created_at: &str,
    delete_after: &str,
) -> Result<SealedEvidence, Error> {
    validate_key_file(key)?;
    let created = parse_time(created_at, "evidence created_at")?;
    let delete = parse_time(delete_after, "evidence delete_after")?;
    if created >= delete {
        return Err(Error::InvalidEvidence(
            "evidence deletion time must follow creation".into(),
        ));
    }
    let protected = ProtectedEvidence {
        created_at: created_at.into(),
        delete_after: delete_after.into(),
        envelope: envelope.clone(),
    };
    let cipher = cipher(key)?;
    let nonce = XNonce::generate();
    let aad = format!("evidence-seal-v1:{}", key.key_id);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: &encode(&protected)?,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| Error::InvalidEvidence("evidence encryption failed".into()))?;
    Ok(SealedEvidence {
        version: "evidence-seal-v1".into(),
        key_id: key.key_id.clone(),
        nonce_hex: hex::encode(nonce),
        ciphertext_hex: hex::encode(ciphertext),
    })
}

pub fn unseal(
    sealed: &SealedEvidence,
    keys: &BTreeMap<String, RecoveryKeyFile>,
    verification_time: &str,
) -> Result<Envelope, Error> {
    if sealed.version != "evidence-seal-v1" {
        return Err(Error::InvalidEvidence(
            "evidence seal version is invalid".into(),
        ));
    }
    let key = keys
        .get(&sealed.key_id)
        .ok_or_else(|| Error::InvalidEvidence("evidence recovery key is unavailable".into()))?;
    validate_key_file(key)?;
    let nonce_bytes: [u8; 24] = hex::decode(&sealed.nonce_hex)
        .map_err(|_| Error::InvalidEvidence("evidence nonce is not hex".into()))?
        .try_into()
        .map_err(|_| Error::InvalidEvidence("evidence nonce is not 24 bytes".into()))?;
    let ciphertext = hex::decode(&sealed.ciphertext_hex)
        .map_err(|_| Error::InvalidEvidence("evidence ciphertext is not hex".into()))?;
    if ciphertext.len() > MAX_BUNDLE_BYTES as usize {
        return Err(Error::SearchExhausted(
            "sealed evidence exceeds 16 MiB".into(),
        ));
    }
    let aad = format!("evidence-seal-v1:{}", key.key_id);
    let nonce = XNonce::from(nonce_bytes);
    let plaintext = cipher(key)?
        .decrypt(
            &nonce,
            Payload {
                msg: &ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| Error::InvalidEvidence("evidence authentication failed".into()))?;
    let protected: ProtectedEvidence = serde_json::from_slice(&plaintext).map_err(|error| {
        Error::InvalidEvidence(format!("protected evidence is invalid: {error}"))
    })?;
    let now = parse_time(verification_time, "verification_time")?;
    if now >= parse_time(&protected.delete_after, "evidence delete_after")? {
        return Err(Error::InvalidEvidence(
            "evidence retention has expired".into(),
        ));
    }
    Ok(protected.envelope)
}

pub fn sign_bundle(mut bundle: StoreBundle, secret_key: &[u8; 32]) -> Result<StoreBundle, Error> {
    bundle.signature.clear();
    validate_id("store id", &bundle.store_id)?;
    let signature = SigningKey::from_bytes(secret_key).sign(&encode(&bundle.unsigned())?);
    bundle.signature = hex::encode(signature.to_bytes());
    Ok(bundle)
}

pub fn sign_bundle_with_key_file(
    bundle: StoreBundle,
    key: &crate::fragments::SigningKeyFile,
) -> Result<StoreBundle, Error> {
    sign_bundle(bundle, &key.secret_bytes()?)
}

impl StoreBundle {
    fn unsigned(&self) -> UnsignedBundle<'_> {
        UnsignedBundle {
            version: &self.version,
            store_id: &self.store_id,
            failure_domain: &self.failure_domain,
            generation: self.generation,
            evidence: &self.evidence,
        }
    }
}

pub fn retrieve(config_path: &Path) -> Result<RetrievedEvidence, Error> {
    let config_bytes = read_regular_bounded(config_path, MAX_CONFIG_BYTES, "evidence config")?;
    let config: EvidencePlaneConfig = serde_json::from_slice(&config_bytes)
        .map_err(|error| Error::InvalidRegistry(format!("evidence config is invalid: {error}")))?;
    validate_config(&config)?;
    let keys = load_keys(&config.recovery_keys)?;
    let mut successes = Vec::new();
    let mut failures = Vec::new();
    for batch in config.stores.chunks(config.max_parallel) {
        let results = thread::scope(|scope| {
            batch
                .iter()
                .map(|store| {
                    scope
                        .spawn(|| fetch_with_retries(store, config.timeout_ms, config.retry_budget))
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|handle| {
                    handle
                        .join()
                        .map_err(|_| Error::InvalidEvidence("store worker panicked".into()))
                        .and_then(|value| value)
                })
                .collect::<Vec<_>>()
        });
        for (store, result) in batch.iter().zip(results) {
            match result.and_then(|bundle| validate_bundle(store, bundle)) {
                Ok(bundle) => successes.push(bundle),
                Err(_) => failures.push(store.store_id.clone()),
            }
        }
    }
    let mut provenance = Vec::new();
    let mut processed = Vec::new();
    for bundle in successes {
        let mut store_envelopes = BTreeMap::new();
        let mut valid = true;
        for sealed in &bundle.evidence {
            match unseal(sealed, &keys, &config.verification_time)
                .and_then(|envelope| Ok((digest(&envelope)?, envelope)))
            {
                Ok((envelope_digest, envelope)) => {
                    store_envelopes.entry(envelope_digest).or_insert(envelope);
                }
                Err(_) => {
                    valid = false;
                    break;
                }
            }
        }
        if !valid {
            failures.push(bundle.store_id);
            continue;
        }
        let bundle_digest = digest(&bundle)?;
        provenance.push(StoreProvenance {
            store_id: bundle.store_id.clone(),
            failure_domain: bundle.failure_domain.clone(),
            generation: bundle.generation,
            bundle_digest,
            evidence_count: bundle.evidence.len(),
        });
        processed.push(store_envelopes);
    }
    if processed.len() < config.required_stores {
        return Err(Error::InsufficientEvidence(
            "authenticated fragment-store quorum was not reached".into(),
        ));
    }
    let mut copies = BTreeMap::<String, usize>::new();
    let mut unique = BTreeMap::new();
    for store_envelopes in processed {
        for (envelope_digest, envelope) in store_envelopes {
            *copies.entry(envelope_digest.clone()).or_default() += 1;
            unique.entry(envelope_digest).or_insert(envelope);
        }
    }
    if copies.values().any(|count| *count < config.required_copies) {
        return Err(Error::InsufficientEvidence(
            "one or more fragments do not meet independent store-copy quorum".into(),
        ));
    }
    let evidence = collect_at(
        unique.into_values().collect(),
        &config.trusted_issuers,
        config.required_fragment_domains,
        &config.component,
        &config.interface_version,
        Some(&config.verification_time),
    )?;
    Ok(to_retrieved(evidence, provenance, failures))
}

fn to_retrieved(
    evidence: CollectedEvidence,
    provenance: Vec<StoreProvenance>,
    failed_stores: Vec<String>,
) -> RetrievedEvidence {
    let envelope_count = evidence.envelopes.len();
    let successful_stores = provenance.len();
    RetrievedEvidence {
        receipt: EvidencePlaneReceipt {
            successful_stores,
            failed_stores,
            provenance,
            fragment_domains: evidence.domains,
            envelope_count,
            verification_audit: evidence.audit,
        },
        envelopes: evidence.envelopes,
    }
}

fn fetch_with_retries(
    store: &StoreConfig,
    timeout_ms: u64,
    retries: u32,
) -> Result<StoreBundle, Error> {
    let mut last = None;
    for _ in 0..=retries {
        match fetch(store, timeout_ms) {
            Ok(bundle) => return Ok(bundle),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| Error::InvalidEvidence("store fetch failed".into())))
}

fn fetch(store: &StoreConfig, timeout_ms: u64) -> Result<StoreBundle, Error> {
    let bytes = match &store.transport {
        StoreTransport::LocalDirectory { path } => {
            read_regular_bounded(&path.join("bundle.json"), MAX_BUNDLE_BYTES, "store bundle")?
        }
        StoreTransport::HttpsBundle {
            url,
            bearer_token_env,
        } => {
            if !url.starts_with("https://") {
                return Err(Error::InvalidRegistry(
                    "remote store URL must use HTTPS".into(),
                ));
            }
            let agent = ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_millis(timeout_ms)))
                .build()
                .new_agent();
            let mut request = agent.get(url);
            if let Some(variable) = bearer_token_env {
                validate_env_name(variable)?;
                let token = std::env::var(variable).map_err(|_| {
                    Error::InvalidRegistry(
                        "store bearer-token environment variable is absent".into(),
                    )
                })?;
                request = request.header("Authorization", &format!("Bearer {token}"));
            }
            let mut response = request.call().map_err(|error| {
                Error::InvalidEvidence(format!("HTTPS store read failed: {error}"))
            })?;
            response
                .body_mut()
                .with_config()
                .limit(MAX_BUNDLE_BYTES)
                .read_to_vec()
                .map_err(|error| {
                    Error::InvalidEvidence(format!("HTTPS store body failed: {error}"))
                })?
        }
    };
    serde_json::from_slice(&bytes)
        .map_err(|error| Error::InvalidEvidence(format!("store bundle is invalid: {error}")))
}

fn validate_bundle(store: &StoreConfig, bundle: StoreBundle) -> Result<StoreBundle, Error> {
    if bundle.version != "fragment-store-v1"
        || bundle.store_id != store.store_id
        || bundle.failure_domain != store.failure_domain
        || bundle.generation < store.minimum_generation
        || bundle.evidence.is_empty()
        || bundle.evidence.len() > 10_000
    {
        return Err(Error::InvalidEvidence(
            "store provenance or generation is invalid".into(),
        ));
    }
    let public: [u8; 32] = hex::decode(&store.public_key)
        .map_err(|_| Error::InvalidRegistry("store public key is not hex".into()))?
        .try_into()
        .map_err(|_| Error::InvalidRegistry("store public key is not 32 bytes".into()))?;
    let signature_bytes = hex::decode(&bundle.signature)
        .map_err(|_| Error::InvalidEvidence("store signature is not hex".into()))?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| Error::InvalidEvidence("store signature length is invalid".into()))?;
    VerifyingKey::from_bytes(&public)
        .map_err(|_| Error::InvalidRegistry("store public key is invalid".into()))?
        .verify_strict(&encode(&bundle.unsigned())?, &signature)
        .map_err(|_| Error::InvalidEvidence("store signature mismatch".into()))?;
    Ok(bundle)
}

fn validate_config(config: &EvidencePlaneConfig) -> Result<(), Error> {
    validate_id("component", &config.component)?;
    validate_id("interface version", &config.interface_version)?;
    parse_time(&config.verification_time, "verification_time")?;
    if config.stores.is_empty()
        || config.stores.len() > MAX_STORES
        || config.required_stores == 0
        || config.required_stores > config.stores.len()
        || config.required_copies == 0
        || config.required_copies > config.required_stores
        || !(1..=MAX_PARALLEL).contains(&config.max_parallel)
        || !(1..=60_000).contains(&config.timeout_ms)
        || config.retry_budget > 3
        || config.required_fragment_domains == 0
    {
        return Err(Error::InvalidRegistry(
            "evidence-plane bounds are invalid".into(),
        ));
    }
    let mut stores = BTreeSet::new();
    let mut domains = BTreeSet::new();
    for store in &config.stores {
        validate_id("store id", &store.store_id)?;
        validate_id("store failure domain", &store.failure_domain)?;
        if !stores.insert(&store.store_id) || !domains.insert(&store.failure_domain) {
            return Err(Error::InvalidRegistry(
                "store ids and administrative failure domains must be unique".into(),
            ));
        }
        if hex::decode(&store.public_key).map_or(true, |bytes| bytes.len() != 32) {
            return Err(Error::InvalidRegistry(
                "store public key must be 32-byte hex".into(),
            ));
        }
    }
    for policy in config.trusted_issuers.values() {
        crate::fragments::validate_issuer_policy(policy)?;
        if matches!(policy, IssuerPolicy::LegacyHmac(_)) {
            return Err(Error::InvalidRegistry(
                "production evidence stores require Ed25519 issuer policies".into(),
            ));
        }
    }
    Ok(())
}

fn load_keys(
    paths: &BTreeMap<String, PathBuf>,
) -> Result<BTreeMap<String, RecoveryKeyFile>, Error> {
    let mut keys = BTreeMap::new();
    for (id, path) in paths {
        let bytes = read_secret(path)?;
        let key: RecoveryKeyFile = serde_json::from_slice(&bytes).map_err(|error| {
            Error::InvalidRegistry(format!("recovery key file is invalid: {error}"))
        })?;
        validate_key_file(&key)?;
        if id != &key.key_id {
            return Err(Error::InvalidRegistry(
                "recovery key id does not match its path mapping".into(),
            ));
        }
        keys.insert(id.clone(), key);
    }
    Ok(keys)
}

fn validate_key_file(key: &RecoveryKeyFile) -> Result<(), Error> {
    if key.version != "evidence-key-v1"
        || hex::decode(&key.key_hex).map_or(true, |bytes| bytes.len() != 32)
    {
        return Err(Error::InvalidRegistry(
            "recovery key file is invalid".into(),
        ));
    }
    validate_id("recovery key id", &key.key_id)?;
    parse_time(&key.created_at, "recovery key created_at")?;
    Ok(())
}

fn cipher(key: &RecoveryKeyFile) -> Result<XChaCha20Poly1305, Error> {
    let mut bytes = hex::decode(&key.key_hex)
        .map_err(|_| Error::InvalidRegistry("recovery key is not hex".into()))?;
    let result = XChaCha20Poly1305::new_from_slice(&bytes)
        .map_err(|_| Error::InvalidRegistry("recovery key length is invalid".into()));
    bytes.zeroize();
    result
}

fn validate_id(label: &str, value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
    {
        return Err(Error::InvalidRegistry(format!("{label} is invalid")));
    }
    Ok(())
}

fn validate_env_name(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(Error::InvalidRegistry(
            "bearer-token environment name is invalid".into(),
        ));
    }
    Ok(())
}

fn parse_time(value: &str, label: &str) -> Result<Timestamp, Error> {
    Timestamp::strptime("%Y-%m-%dT%H:%M:%S%:z", value)
        .map_err(|_| Error::InvalidRegistry(format!("{label} is invalid")))
}

fn read_regular_bounded(path: &Path, max: u64, label: &str) -> Result<Vec<u8>, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > max {
        return Err(Error::InvalidEvidence(format!(
            "{label} must be a bounded regular file"
        )));
    }
    Ok(fs::read(path)?)
}

fn read_secret(path: &Path) -> Result<Vec<u8>, Error> {
    let bytes = read_regular_bounded(path, 4096, "recovery key")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::symlink_metadata(path)?.permissions().mode() & 0o077 != 0 {
            return Err(Error::InvalidRegistry(
                "recovery key permissions must exclude group and other".into(),
            ));
        }
    }
    Ok(bytes)
}

fn write_secret_new(path: &Path, data: &[u8]) -> Result<(), Error> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(())
}

fn write_regular_new(path: &Path, data: &[u8]) -> Result<(), Error> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(())
}
