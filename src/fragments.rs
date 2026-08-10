use std::collections::{BTreeMap, BTreeSet};

use hmac::{Hmac, KeyInit, Mac};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::canonical::{digest, encode};
use crate::model::{Error, FragmentContent};

type HmacSha256 = Hmac<Sha256>;

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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IssuerPolicy {
    pub hmac_sha256_key: String,
    pub failure_domain: String,
}

#[derive(Debug)]
pub struct CollectedEvidence {
    pub envelopes: Vec<Envelope>,
    pub domains: Vec<String>,
}

pub fn collect(
    mut envelopes: Vec<Envelope>,
    trusted: &BTreeMap<String, IssuerPolicy>,
    required_domains: usize,
    component: &str,
    interface_version: &str,
) -> Result<CollectedEvidence, Error> {
    let mut identities = BTreeSet::new();
    let mut content_digests = BTreeSet::new();
    let mut domains = BTreeSet::new();

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
        if envelope.failure_domain != policy.failure_domain {
            return Err(Error::InvalidEvidence(
                "fragment failure domain violates issuer policy".into(),
            ));
        }
        let key: [u8; 32] = hex::decode(&policy.hmac_sha256_key)
            .map_err(|_| Error::InvalidRegistry("issuer key is not hex".into()))?
            .try_into()
            .map_err(|_| Error::InvalidRegistry("issuer key is not 32 bytes".into()))?;
        if digest(&envelope.content)? != envelope.content_digest {
            return Err(Error::InvalidEvidence(
                "fragment content digest mismatch".into(),
            ));
        }
        let provided_signature = hex::decode(&envelope.signature)
            .map_err(|_| Error::InvalidEvidence("fragment signature is not hex".into()))?;
        let mut mac = HmacSha256::new_from_slice(&key)
            .map_err(|error| Error::InvalidRegistry(error.to_string()))?;
        mac.update(&encode(&envelope.unsigned())?);
        if mac.verify_slice(&provided_signature).is_err() {
            return Err(Error::InvalidEvidence("fragment signature mismatch".into()));
        }
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
    })
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
