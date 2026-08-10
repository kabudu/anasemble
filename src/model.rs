use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum Error {
    InvalidRegistry(String),
    InvalidEvidence(String),
    InsufficientEvidence(String),
    ContradictoryEvidence(String),
    ArtifactPresent(String),
    SearchExhausted(String),
    CheckerRejected(String),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRegistry(message)
            | Self::InvalidEvidence(message)
            | Self::InsufficientEvidence(message)
            | Self::ContradictoryEvidence(message)
            | Self::ArtifactPresent(message)
            | Self::SearchExhausted(message)
            | Self::CheckerRejected(message) => formatter.write_str(message),
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Json(error) => write!(formatter, "JSON error: {error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RefusalCode {
    InvalidRegistry,
    InvalidEvidence,
    InsufficientEvidence,
    ContradictoryEvidence,
    ArtifactPresent,
    SearchExhausted,
    CheckerRejected,
    InternalError,
}

impl Error {
    #[must_use]
    pub fn refusal_code(&self) -> RefusalCode {
        match self {
            Self::InvalidRegistry(_) | Self::Io(_) | Self::Json(_) => RefusalCode::InvalidRegistry,
            Self::InvalidEvidence(_) => RefusalCode::InvalidEvidence,
            Self::InsufficientEvidence(_) => RefusalCode::InsufficientEvidence,
            Self::ContradictoryEvidence(_) => RefusalCode::ContradictoryEvidence,
            Self::ArtifactPresent(_) => RefusalCode::ArtifactPresent,
            Self::SearchExhausted(_) => RefusalCode::SearchExhausted,
            Self::CheckerRejected(_) => RefusalCode::CheckerRejected,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Grammar {
    pub version: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub states: Vec<String>,
    pub initial_state: String,
    pub max_candidates: u64,
}

impl Grammar {
    pub fn validate(&self) -> Result<(), Error> {
        validate_symbols("inputs", &self.inputs)?;
        validate_symbols("outputs", &self.outputs)?;
        validate_symbols("states", &self.states)?;
        if self.version.is_empty() {
            return Err(Error::InvalidRegistry("grammar version is empty".into()));
        }
        if !self.states.contains(&self.initial_state) {
            return Err(Error::InvalidRegistry(
                "initial state is not declared".into(),
            ));
        }
        if !(1..=1_000_000).contains(&self.max_candidates) {
            return Err(Error::InvalidRegistry(
                "max_candidates must be between 1 and 1000000".into(),
            ));
        }
        Ok(())
    }
}

fn validate_symbols(name: &str, values: &[String]) -> Result<(), Error> {
    if values.is_empty() || values.iter().any(String::is_empty) {
        return Err(Error::InvalidRegistry(format!(
            "{name} must contain non-empty symbols"
        )));
    }
    let unique: BTreeSet<_> = values.iter().collect();
    if unique.len() != values.len() {
        return Err(Error::InvalidRegistry(format!(
            "{name} contains duplicate symbols"
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub state: String,
    pub input: String,
    pub next_state: String,
    pub output: String,
}

impl Transition {
    pub fn validate(&self, grammar: &Grammar) -> Result<(), Error> {
        if !grammar.states.contains(&self.state) || !grammar.states.contains(&self.next_state) {
            return Err(Error::InvalidEvidence(
                "transition contains an unknown state".into(),
            ));
        }
        if !grammar.inputs.contains(&self.input) || !grammar.outputs.contains(&self.output) {
            return Err(Error::InvalidEvidence(
                "transition contains an unknown input or output".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub component: String,
    pub interface_version: String,
    pub grammar: Grammar,
    pub transitions: Vec<Transition>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FragmentContent {
    Transition {
        state: String,
        input: String,
        next_state: String,
        output: String,
    },
    StatePolicy {
        states: Vec<String>,
        initial_state: String,
    },
    Trace {
        initial_state: String,
        inputs: Vec<String>,
        outputs: Vec<String>,
    },
}

impl FragmentContent {
    #[must_use]
    pub fn transition(&self) -> Option<Transition> {
        match self {
            Self::Transition {
                state,
                input,
                next_state,
                output,
            } => Some(Transition {
                state: state.clone(),
                input: input.clone(),
                next_state: next_state.clone(),
                output: output.clone(),
            }),
            _ => None,
        }
    }
}
