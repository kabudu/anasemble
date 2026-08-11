use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::Path;

use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Generate, Key};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hmac::{Hmac, KeyInit, Mac};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::canonical::{digest, encode};
use crate::model::{Error, FragmentContent};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SigningKeyFile {
    pub version: String,
    pub key_id: String,
    pub secret_key_hex: String,
    pub public_key_hex: String,
    pub created_at: String,
}

pub fn create_signing_key(path: &Path, key_id: &str, created_at: &str) -> Result<(), Error> {
    validate_key_id(key_id)?;
    parse_time(created_at, "signing key created_at")?;
    let random = Key::<XChaCha20Poly1305>::generate();
    let secret: [u8; 32] = random.into();
    let signing = SigningKey::from_bytes(&secret);
    let file = SigningKeyFile {
        version: "ed25519-key-v1".into(),
        key_id: key_id.into(),
        secret_key_hex: hex::encode(secret),
        public_key_hex: hex::encode(signing.verifying_key().to_bytes()),
        created_at: created_at.into(),
    };
    write_secret_new(path, &encode(&file)?)
}

pub fn read_signing_key(path: &Path) -> Result<SigningKeyFile, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 4096 {
        return Err(Error::InvalidRegistry(
            "signing key must be a bounded regular file".into(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(Error::InvalidRegistry(
                "signing key permissions must exclude group and other".into(),
            ));
        }
    }
    let file: SigningKeyFile = serde_json::from_slice(&fs::read(path)?)
        .map_err(|error| Error::InvalidRegistry(format!("signing key is invalid: {error}")))?;
    validate_signing_key(&file)?;
    Ok(file)
}

pub fn sign_with_key_file(envelope: Envelope, key: &SigningKeyFile) -> Result<Envelope, Error> {
    validate_signing_key(key)?;
    let secret = key.secret_bytes()?;
    sign_ed25519(envelope, &key.key_id, &secret)
}

impl SigningKeyFile {
    pub fn secret_bytes(&self) -> Result<[u8; 32], Error> {
        validate_signing_key(self)?;
        hex::decode(&self.secret_key_hex)
            .map_err(|_| Error::InvalidRegistry("signing secret is not hex".into()))?
            .try_into()
            .map_err(|_| Error::InvalidRegistry("signing secret is not 32 bytes".into()))
    }
}

fn validate_signing_key(key: &SigningKeyFile) -> Result<(), Error> {
    if key.version != "ed25519-key-v1" {
        return Err(Error::InvalidRegistry(
            "signing key version is invalid".into(),
        ));
    }
    validate_key_id(&key.key_id)?;
    parse_time(&key.created_at, "signing key created_at")?;
    let secret: [u8; 32] = hex::decode(&key.secret_key_hex)
        .map_err(|_| Error::InvalidRegistry("signing secret is not hex".into()))?
        .try_into()
        .map_err(|_| Error::InvalidRegistry("signing secret is not 32 bytes".into()))?;
    if hex::encode(SigningKey::from_bytes(&secret).verifying_key().to_bytes()) != key.public_key_hex
    {
        return Err(Error::InvalidRegistry(
            "signing public key does not match its secret".into(),
        ));
    }
    Ok(())
}

fn write_secret_new(path: &Path, data: &[u8]) -> Result<(), Error> {
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FragmentKind {
    Contract,
    Trace,
    StateSchema,
    MetamorphicProperty,
    NegativeCase,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub kind: FragmentKind,
    pub component: String,
    pub interface_version: String,
    pub issuer: String,
    pub failure_domain: String,
    pub issued_at: String,
    pub sequence: u64,
    pub content_digest: String,
    pub dependencies: Vec<String>,
    pub content: FragmentContent,
    pub signature: String,
}

#[derive(Serialize)]
struct UnsignedEnvelope<'a> {
    kind: FragmentKind,
    component: &'a str,
    interface_version: &'a str,
    issuer: &'a str,
    failure_domain: &'a str,
    issued_at: &'a str,
    sequence: u64,
    content_digest: &'a str,
    dependencies: &'a [String],
    content: &'a FragmentContent,
}

impl Envelope {
    fn unsigned(&self) -> UnsignedEnvelope<'_> {
        UnsignedEnvelope {
            kind: self.kind,
            component: &self.component,
            interface_version: &self.interface_version,
            issuer: &self.issuer,
            failure_domain: &self.failure_domain,
            issued_at: &self.issued_at,
            sequence: self.sequence,
            content_digest: &self.content_digest,
            dependencies: &self.dependencies,
            content: &self.content,
        }
    }
}

pub fn sign(mut envelope: Envelope, key: &[u8; 32]) -> Result<Envelope, Error> {
    envelope.content_digest = digest(&envelope.content)?;
    envelope.signature.clear();
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|error| Error::InvalidRegistry(error.to_string()))?;
    mac.update(&encode(&envelope.unsigned())?);
    envelope.signature = hex::encode(mac.finalize().into_bytes());
    Ok(envelope)
}

pub fn sign_ed25519(
    mut envelope: Envelope,
    key_id: &str,
    secret_key: &[u8; 32],
) -> Result<Envelope, Error> {
    validate_key_id(key_id)?;
    envelope.content_digest = digest(&envelope.content)?;
    envelope.signature.clear();
    let signature = SigningKey::from_bytes(secret_key).sign(&encode(&envelope.unsigned())?);
    envelope.signature = format!("ed25519:{key_id}:{}", hex::encode(signature.to_bytes()));
    Ok(envelope)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum IssuerPolicy {
    LegacyHmac(LegacyHmacPolicy),
    Ed25519(Ed25519IssuerPolicy),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyHmacPolicy {
    pub hmac_sha256_key: String,
    pub failure_domain: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ed25519IssuerPolicy {
    pub failure_domain: String,
    pub minimum_sequence: u64,
    pub keys: Vec<Ed25519KeyPolicy>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ed25519KeyPolicy {
    pub key_id: String,
    pub public_key: String,
    pub not_before: String,
    pub not_after: String,
    #[serde(default)]
    pub revoked_at: Option<String>,
}

impl IssuerPolicy {
    #[must_use]
    pub fn failure_domain(&self) -> &str {
        match self {
            Self::LegacyHmac(policy) => &policy.failure_domain,
            Self::Ed25519(policy) => &policy.failure_domain,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct VerificationAudit {
    pub issuer: String,
    pub key_id: String,
    pub sequence: u64,
    pub content_digest: String,
    pub decision: &'static str,
}

#[derive(Debug)]
pub struct CollectedEvidence {
    pub envelopes: Vec<Envelope>,
    pub domains: Vec<String>,
    pub audit: Vec<VerificationAudit>,
}

pub fn collect(
    envelopes: Vec<Envelope>,
    trusted: &BTreeMap<String, IssuerPolicy>,
    required_domains: usize,
    component: &str,
    interface_version: &str,
) -> Result<CollectedEvidence, Error> {
    collect_at(
        envelopes,
        trusted,
        required_domains,
        component,
        interface_version,
        None,
    )
}

pub fn collect_at(
    mut envelopes: Vec<Envelope>,
    trusted: &BTreeMap<String, IssuerPolicy>,
    required_domains: usize,
    component: &str,
    interface_version: &str,
    verification_time: Option<&str>,
) -> Result<CollectedEvidence, Error> {
    let verification_time = verification_time
        .map(|value| parse_time(value, "verification_time"))
        .transpose()?;
    let mut identities = BTreeSet::new();
    let mut content_digests = BTreeSet::new();
    let mut domains = BTreeSet::new();
    let mut audit = Vec::with_capacity(envelopes.len());

    for envelope in &envelopes {
        let kind_matches_content = matches!(
            (&envelope.kind, &envelope.content),
            (FragmentKind::Contract, FragmentContent::Transition { .. })
                | (FragmentKind::Trace, FragmentContent::Trace { .. })
                | (
                    FragmentKind::StateSchema,
                    FragmentContent::StatePolicy { .. }
                )
                | (
                    FragmentKind::NegativeCase,
                    FragmentContent::NegativeCase { .. }
                )
                | (
                    FragmentKind::MetamorphicProperty,
                    FragmentContent::MetamorphicProperty { .. }
                )
        );
        if !kind_matches_content {
            return Err(Error::InvalidEvidence(
                "fragment kind does not match its M0 content schema".into(),
            ));
        }
        if envelope.component != component || envelope.interface_version != interface_version {
            return Err(Error::InvalidEvidence(
                "fragment targets another component or interface".into(),
            ));
        }
        Timestamp::strptime("%Y-%m-%dT%H:%M:%S%:z", &envelope.issued_at)
            .map_err(|_| Error::InvalidEvidence("issued_at is not canonical RFC3339".into()))?;
        let policy = trusted
            .get(&envelope.issuer)
            .ok_or_else(|| Error::InvalidEvidence("fragment issuer is not trusted".into()))?;
        if envelope.failure_domain != policy.failure_domain() {
            return Err(Error::InvalidEvidence(
                "fragment failure domain violates issuer policy".into(),
            ));
        }
        if digest(&envelope.content)? != envelope.content_digest {
            return Err(Error::InvalidEvidence(
                "fragment content digest mismatch".into(),
            ));
        }
        let key_id = verify_signature(envelope, policy, verification_time)?;
        if !identities.insert((&envelope.issuer, envelope.sequence)) {
            return Err(Error::InvalidEvidence(
                "fragment issuer equivocation or replay".into(),
            ));
        }
        if !content_digests.insert(envelope.content_digest.clone()) {
            return Err(Error::InvalidEvidence(
                "duplicate fragment content digest".into(),
            ));
        }
        domains.insert(envelope.failure_domain.clone());
        audit.push(VerificationAudit {
            issuer: envelope.issuer.clone(),
            key_id,
            sequence: envelope.sequence,
            content_digest: envelope.content_digest.clone(),
            decision: "accepted",
        });
    }

    let graph: BTreeMap<_, _> = envelopes
        .iter()
        .map(|item| (item.content_digest.as_str(), item.dependencies.as_slice()))
        .collect();
    for dependencies in graph.values() {
        if dependencies
            .iter()
            .any(|item| !graph.contains_key(item.as_str()))
        {
            return Err(Error::InvalidEvidence(
                "fragment dependency is unavailable".into(),
            ));
        }
    }
    reject_cycles(&graph)?;
    if domains.len() < required_domains {
        return Err(Error::InsufficientEvidence(
            "insufficient independent failure domains".into(),
        ));
    }
    if !envelopes
        .iter()
        .any(|item| item.kind == FragmentKind::Contract)
        || !envelopes
            .iter()
            .any(|item| item.kind == FragmentKind::StateSchema)
    {
        return Err(Error::InsufficientEvidence(
            "contract and state schema fragments are mandatory".into(),
        ));
    }
    envelopes.sort_by(|left, right| {
        (&left.issuer, left.sequence, &left.content_digest).cmp(&(
            &right.issuer,
            right.sequence,
            &right.content_digest,
        ))
    });
    Ok(CollectedEvidence {
        envelopes,
        domains: domains.into_iter().collect(),
        audit,
    })
}

fn verify_signature(
    envelope: &Envelope,
    policy: &IssuerPolicy,
    verification_time: Option<Timestamp>,
) -> Result<String, Error> {
    match policy {
        IssuerPolicy::LegacyHmac(policy) => {
            let key: [u8; 32] = hex::decode(&policy.hmac_sha256_key)
                .map_err(|_| Error::InvalidRegistry("issuer key is not hex".into()))?
                .try_into()
                .map_err(|_| Error::InvalidRegistry("issuer key is not 32 bytes".into()))?;
            let provided_signature = hex::decode(&envelope.signature)
                .map_err(|_| Error::InvalidEvidence("fragment signature is not hex".into()))?;
            let mut mac = HmacSha256::new_from_slice(&key)
                .map_err(|error| Error::InvalidRegistry(error.to_string()))?;
            mac.update(&encode(&envelope.unsigned())?);
            mac.verify_slice(&provided_signature)
                .map_err(|_| Error::InvalidEvidence("fragment signature mismatch".into()))?;
            Ok("legacy-hmac".into())
        }
        IssuerPolicy::Ed25519(policy) => {
            if envelope.sequence < policy.minimum_sequence {
                return Err(Error::InvalidEvidence(
                    "fragment sequence is below the issuer replay floor".into(),
                ));
            }
            let mut parts = envelope.signature.split(':');
            if parts.next() != Some("ed25519") {
                return Err(Error::InvalidEvidence(
                    "production issuer requires an Ed25519 signature".into(),
                ));
            }
            let key_id = parts
                .next()
                .ok_or_else(|| Error::InvalidEvidence("signature key id is absent".into()))?;
            let signature_hex = parts
                .next()
                .ok_or_else(|| Error::InvalidEvidence("signature bytes are absent".into()))?;
            if parts.next().is_some() {
                return Err(Error::InvalidEvidence("signature format is invalid".into()));
            }
            let key = policy
                .keys
                .iter()
                .find(|key| key.key_id == key_id)
                .ok_or_else(|| Error::InvalidEvidence("signature key is not trusted".into()))?;
            validate_key_policy(key)?;
            let issued_at = parse_time(&envelope.issued_at, "fragment issued_at")?;
            let not_before = parse_time(&key.not_before, "key not_before")?;
            let not_after = parse_time(&key.not_after, "key not_after")?;
            if issued_at < not_before || issued_at > not_after {
                return Err(Error::InvalidEvidence(
                    "fragment was signed outside the key validity interval".into(),
                ));
            }
            if verification_time.is_some_and(|time| time < not_before || time > not_after) {
                return Err(Error::InvalidEvidence(
                    "signature key is outside its verification validity interval".into(),
                ));
            }
            if key.revoked_at.is_some() {
                return Err(Error::InvalidEvidence("signature key is revoked".into()));
            }
            let public: [u8; 32] = hex::decode(&key.public_key)
                .map_err(|_| Error::InvalidRegistry("Ed25519 public key is not hex".into()))?
                .try_into()
                .map_err(|_| Error::InvalidRegistry("Ed25519 public key is not 32 bytes".into()))?;
            let verifying = VerifyingKey::from_bytes(&public)
                .map_err(|_| Error::InvalidRegistry("Ed25519 public key is invalid".into()))?;
            let signature_bytes = hex::decode(signature_hex)
                .map_err(|_| Error::InvalidEvidence("Ed25519 signature is not hex".into()))?;
            let signature = Signature::from_slice(&signature_bytes).map_err(|_| {
                Error::InvalidEvidence("Ed25519 signature length is invalid".into())
            })?;
            verifying
                .verify_strict(&encode(&envelope.unsigned())?, &signature)
                .map_err(|_| Error::InvalidEvidence("fragment signature mismatch".into()))?;
            Ok(key_id.into())
        }
    }
}

pub fn validate_issuer_policy(policy: &IssuerPolicy) -> Result<(), Error> {
    if policy.failure_domain().is_empty() {
        return Err(Error::InvalidRegistry(
            "issuer failure domain is empty".into(),
        ));
    }
    match policy {
        IssuerPolicy::LegacyHmac(policy) => {
            if hex::decode(&policy.hmac_sha256_key).map_or(true, |key| key.len() != 32) {
                return Err(Error::InvalidRegistry(
                    "legacy issuer requires a 32-byte hex HMAC key".into(),
                ));
            }
        }
        IssuerPolicy::Ed25519(policy) => {
            if policy.keys.is_empty() || policy.keys.len() > 16 {
                return Err(Error::InvalidRegistry(
                    "production issuer requires between 1 and 16 keys".into(),
                ));
            }
            let mut ids = BTreeSet::new();
            for key in &policy.keys {
                validate_key_policy(key)?;
                if !ids.insert(&key.key_id) {
                    return Err(Error::InvalidRegistry(
                        "issuer key ids are duplicated".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_key_policy(key: &Ed25519KeyPolicy) -> Result<(), Error> {
    validate_key_id(&key.key_id)?;
    if hex::decode(&key.public_key).map_or(true, |bytes| bytes.len() != 32) {
        return Err(Error::InvalidRegistry(
            "Ed25519 public key must be 32-byte hex".into(),
        ));
    }
    let not_before = parse_time(&key.not_before, "key not_before")?;
    let not_after = parse_time(&key.not_after, "key not_after")?;
    if not_before > not_after {
        return Err(Error::InvalidRegistry(
            "key validity interval is inverted".into(),
        ));
    }
    if let Some(revoked_at) = &key.revoked_at {
        parse_time(revoked_at, "key revoked_at")?;
    }
    Ok(())
}

fn validate_key_id(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err(Error::InvalidRegistry("signature key id is invalid".into()));
    }
    Ok(())
}

fn parse_time(value: &str, label: &str) -> Result<Timestamp, Error> {
    Timestamp::strptime("%Y-%m-%dT%H:%M:%S%:z", value)
        .map_err(|_| Error::InvalidRegistry(format!("{label} is invalid")))
}

fn reject_cycles(graph: &BTreeMap<&str, &[String]>) -> Result<(), Error> {
    fn visit<'a>(
        node: &'a str,
        graph: &BTreeMap<&'a str, &'a [String]>,
        visiting: &mut BTreeSet<&'a str>,
        visited: &mut BTreeSet<&'a str>,
    ) -> Result<(), Error> {
        if visiting.contains(node) {
            return Err(Error::InvalidEvidence("fragment dependency cycle".into()));
        }
        if visited.contains(node) {
            return Ok(());
        }
        visiting.insert(node);
        for dependency in graph[node] {
            visit(dependency, graph, visiting, visited)?;
        }
        visiting.remove(node);
        visited.insert(node);
        Ok(())
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for node in graph.keys() {
        visit(node, graph, &mut visiting, &mut visited)?;
    }
    Ok(())
}
