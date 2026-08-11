use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::model::Error;

const MAX_ENDPOINTS: usize = 256;
const MAX_DEPENDENCIES: usize = 64;
const MAX_IDENTIFIER_BYTES: usize = 128;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ServiceManifest {
    pub version: String,
    pub component: String,
    pub interface_version: String,
    pub http: HttpContract,
    pub effects: Vec<Effect>,
    pub state_dependencies: Vec<StateDependency>,
    pub limits: ServiceLimits,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HttpContract {
    pub endpoints: Vec<HttpEndpoint>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
pub struct HttpEndpoint {
    pub method: HttpMethod,
    pub path: String,
    pub request_schema_sha256: String,
    pub response_schema_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Delete,
    Get,
    Head,
    Patch,
    Post,
    Put,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub kind: EffectKind,
    pub target: String,
    pub access: EffectAccess,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    Clock,
    Filesystem,
    Network,
    Randomness,
    State,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum EffectAccess {
    Read,
    ReadWrite,
    Write,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
pub struct StateDependency {
    pub name: String,
    pub adapter: StateAdapter,
    pub consistency: Consistency,
    pub required: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum StateAdapter {
    Filesystem,
    ObjectStore,
    Postgres,
    Queue,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum Consistency {
    Eventual,
    Snapshot,
    Transactional,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ServiceLimits {
    pub request_bytes: u64,
    pub response_bytes: u64,
    pub wall_time_ms: u64,
    pub concurrent_requests: u32,
}

impl ServiceManifest {
    pub fn validate(&self) -> Result<(), Error> {
        if self.version != "service-v1" {
            return Err(invalid("service manifest version must be service-v1"));
        }
        validate_identifier("component", &self.component)?;
        validate_identifier("interface_version", &self.interface_version)?;
        if self.http.endpoints.is_empty() || self.http.endpoints.len() > MAX_ENDPOINTS {
            return Err(invalid(
                "HTTP endpoints must contain between 1 and 256 entries",
            ));
        }
        let mut routes = BTreeSet::new();
        for endpoint in &self.http.endpoints {
            validate_path(&endpoint.path)?;
            validate_digest("request schema", &endpoint.request_schema_sha256)?;
            validate_digest("response schema", &endpoint.response_schema_sha256)?;
            if !routes.insert((endpoint.method, endpoint.path.as_str())) {
                return Err(invalid("HTTP method and path pairs must be unique"));
            }
        }
        if self.effects.len() > MAX_DEPENDENCIES || self.state_dependencies.len() > MAX_DEPENDENCIES
        {
            return Err(invalid("effect or state dependency count exceeds 64"));
        }
        let mut effects = BTreeSet::new();
        for effect in &self.effects {
            validate_identifier("effect target", &effect.target)?;
            if !effects.insert(effect) {
                return Err(invalid("effect declarations must be unique"));
            }
        }
        let mut names = BTreeSet::new();
        for dependency in &self.state_dependencies {
            validate_identifier("state dependency name", &dependency.name)?;
            if !names.insert(dependency.name.as_str()) {
                return Err(invalid("state dependency names must be unique"));
            }
        }
        if self.effects.iter().any(|effect| {
            effect.kind == EffectKind::State
                && !self
                    .state_dependencies
                    .iter()
                    .any(|dependency| dependency.name == effect.target)
        }) || self.state_dependencies.iter().any(|dependency| {
            !self
                .effects
                .iter()
                .any(|effect| effect.kind == EffectKind::State && effect.target == dependency.name)
        }) {
            return Err(invalid(
                "state effects and state dependencies must reference each other exactly",
            ));
        }
        if !(1..=67_108_864).contains(&self.limits.request_bytes)
            || !(1..=67_108_864).contains(&self.limits.response_bytes)
            || !(1..=300_000).contains(&self.limits.wall_time_ms)
            || !(1..=10_000).contains(&self.limits.concurrent_requests)
        {
            return Err(invalid(
                "service resource limits are zero or exceed safety maxima",
            ));
        }
        Ok(())
    }
}

fn validate_identifier(label: &str, value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
    {
        return Err(invalid(&format!(
            "{label} is empty, too long, or contains unsafe characters"
        )));
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), Error> {
    if !path.starts_with('/')
        || path.len() > 1024
        || !path
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'\\')
        || path.contains('?')
        || path.contains('#')
        || path.contains("//")
        || path
            .split('/')
            .any(|segment| segment == "." || segment == "..")
    {
        return Err(invalid(
            "HTTP paths must be bounded absolute paths without query, fragment, empty interior, or traversal segments",
        ));
    }
    Ok(())
}

fn validate_digest(label: &str, digest: &str) -> Result<(), Error> {
    if digest.len() != 64 || hex::decode(digest).map_or(true, |bytes| bytes.len() != 32) {
        return Err(invalid(&format!("{label} digest must be SHA-256 hex")));
    }
    Ok(())
}

fn invalid(message: &str) -> Error {
    Error::InvalidRegistry(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> ServiceManifest {
        ServiceManifest {
            version: "service-v1".into(),
            component: "orders".into(),
            interface_version: "1".into(),
            http: HttpContract {
                endpoints: vec![HttpEndpoint {
                    method: HttpMethod::Post,
                    path: "/orders".into(),
                    request_schema_sha256: "11".repeat(32),
                    response_schema_sha256: "22".repeat(32),
                }],
            },
            effects: vec![Effect {
                kind: EffectKind::State,
                target: "orders-db".into(),
                access: EffectAccess::ReadWrite,
            }],
            state_dependencies: vec![StateDependency {
                name: "orders-db".into(),
                adapter: StateAdapter::Postgres,
                consistency: Consistency::Transactional,
                required: true,
            }],
            limits: ServiceLimits {
                request_bytes: 1_048_576,
                response_bytes: 1_048_576,
                wall_time_ms: 5_000,
                concurrent_requests: 64,
            },
        }
    }

    #[test]
    fn valid_manifest_is_accepted() {
        manifest().validate().unwrap();
    }

    #[test]
    fn duplicate_route_and_unbounded_limit_are_rejected() {
        let mut duplicate = manifest();
        duplicate
            .http
            .endpoints
            .push(duplicate.http.endpoints[0].clone());
        assert!(duplicate.validate().is_err());
        let mut unbounded = manifest();
        unbounded.limits.wall_time_ms = u64::MAX;
        assert!(unbounded.validate().is_err());
    }

    #[test]
    fn traversal_and_unknown_fields_are_rejected() {
        let mut traversal = manifest();
        traversal.http.endpoints[0].path = "/orders/../secrets".into();
        assert!(traversal.validate().is_err());
        let mut value = serde_json::to_value(manifest()).unwrap();
        value["ambient_authority"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<ServiceManifest>(value).is_err());
    }
}
