//! Replayable behavioural traces. Replay denies side effects unless a mock is named.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use anasemble_core::{BoundedText, Budget, Digest, TraceId, encode_canonical_cbor};
use anasemble_evidence::{SignatureBlock, SigningKey};
use ed25519_dalek::{Signature, Signer, SigningKey as Ed25519SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Trace schema version.
pub const SCHEMA_VERSION: u16 = 1;
/// Capability name for pure, side-effect-free steps.
pub const PURE_CAPABILITY: &str = "pure";

/// Fail-closed trace error.
#[derive(Debug, Error)]
pub enum TraceError {
    #[error(transparent)]
    Core(#[from] anasemble_core::CoreError),
    #[error(transparent)]
    Id(#[from] anasemble_core::IdError),
    #[error("{0}")]
    Protocol(&'static str),
    #[error("{0}")]
    Crypto(&'static str),
    #[error("budget exhausted")]
    Budget,
    #[error("side effect denied: {0}")]
    SideEffectDenied(String),
}

/// One ordered step.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TraceStep {
    /// Zero-based index.
    pub index: u64,
    /// Input digest.
    pub input_digest: Digest,
    /// Output digest.
    pub output_digest: Digest,
    /// State digest before the step.
    pub before_state: Digest,
    /// State digest after the step.
    pub after_state: Digest,
    /// Monotone logical clock.
    pub logical_clock: u64,
    /// Causal parent step indexes.
    pub causal_parents: Vec<u64>,
    /// Named capability. Replay denies anything except `pure` without a mock.
    pub capability: BoundedText,
}

/// Declared side effect. Replay must mock it or refuse.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SideEffectClaim {
    /// Capability that would perform the effect.
    pub capability: BoundedText,
    /// Digest of the effect description.
    pub payload_digest: Digest,
}

/// Coverage recorded with the trace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CoverageClaim {
    /// Steps retained for training.
    pub trained_steps: u32,
    /// Steps held out.
    pub held_out_steps: u32,
}

/// Signed behavioural trace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TraceV1 {
    /// Schema version.
    pub schema_version: u16,
    /// Trace identity.
    pub trace_id: TraceId,
    /// Contract this trace exercises.
    pub contract_ref: BoundedText,
    /// Starting state digest.
    pub initial_state: Digest,
    /// Ordered steps.
    pub steps: Vec<TraceStep>,
    /// Side effects claimed by the run.
    pub side_effect_claims: Vec<SideEffectClaim>,
    /// Terminal state digest.
    pub terminal_state: Digest,
    /// Coverage split.
    pub coverage: CoverageClaim,
    /// Digest of the unsigned trace.
    pub digest: Digest,
}

#[derive(Serialize)]
struct UnsignedTrace<'a> {
    schema_version: u16,
    trace_id: TraceId,
    contract_ref: &'a BoundedText,
    initial_state: Digest,
    steps: &'a [TraceStep],
    side_effect_claims: &'a [SideEffectClaim],
    terminal_state: Digest,
    coverage: &'a CoverageClaim,
}

impl TraceV1 {
    fn unsigned(&self) -> UnsignedTrace<'_> {
        UnsignedTrace {
            schema_version: self.schema_version,
            trace_id: self.trace_id,
            contract_ref: &self.contract_ref,
            initial_state: self.initial_state,
            steps: &self.steps,
            side_effect_claims: &self.side_effect_claims,
            terminal_state: self.terminal_state,
            coverage: &self.coverage,
        }
    }

    /// Canonical digest of the unsigned trace.
    pub fn compute_digest(&self) -> Result<Digest, TraceError> {
        Ok(Digest::sha256(&encode_canonical_cbor(&self.unsigned())?))
    }

    fn bind_digest(mut self) -> Result<Self, TraceError> {
        self.digest = self.compute_digest()?;
        Ok(self)
    }
}

/// Records steps in order. Clocks must increase.
#[derive(Clone, Debug)]
pub struct Recorder {
    trace_id: TraceId,
    contract_ref: BoundedText,
    initial_state: Digest,
    steps: Vec<TraceStep>,
    side_effects: Vec<SideEffectClaim>,
    last_clock: Option<u64>,
    last_state: Digest,
}

impl Recorder {
    /// Start a trace.
    pub fn new(contract_ref: &str, initial_state: Digest) -> Result<Self, TraceError> {
        Ok(Self {
            trace_id: TraceId::generate(),
            contract_ref: BoundedText::new(contract_ref)?,
            initial_state,
            steps: Vec::new(),
            side_effects: Vec::new(),
            last_clock: None,
            last_state: initial_state,
        })
    }

    /// Append one step. `before_state` must match the previous after-state.
    #[allow(clippy::too_many_arguments)]
    pub fn record_step(
        &mut self,
        input_digest: Digest,
        output_digest: Digest,
        after_state: Digest,
        logical_clock: u64,
        causal_parents: Vec<u64>,
        capability: &str,
        budget: Budget,
    ) -> Result<(), TraceError> {
        if self.steps.len() as u32 >= budget.max_nodes() {
            return Err(TraceError::Budget);
        }
        if self.last_clock.is_some_and(|clock| logical_clock <= clock) {
            return Err(TraceError::Protocol("logical clock must increase"));
        }
        let index = u64::try_from(self.steps.len()).expect("step count fits u64");
        for parent in &causal_parents {
            if *parent >= index {
                return Err(TraceError::Protocol("causal parent is not prior"));
            }
        }
        let capability = BoundedText::new(capability)?;
        if capability.as_str() != PURE_CAPABILITY {
            self.side_effects.push(SideEffectClaim {
                capability: capability.clone(),
                payload_digest: output_digest,
            });
        }
        self.steps.push(TraceStep {
            index,
            input_digest,
            output_digest,
            before_state: self.last_state,
            after_state,
            logical_clock,
            causal_parents,
            capability,
        });
        self.last_clock = Some(logical_clock);
        self.last_state = after_state;
        Ok(())
    }

    /// Finish the trace. Coverage treats all recorded steps as trained until partitioned.
    pub fn finish(self) -> Result<TraceV1, TraceError> {
        if self.steps.is_empty() {
            return Err(TraceError::Protocol("trace has no steps"));
        }
        let trained = u32::try_from(self.steps.len()).expect("step count fits u32");
        TraceV1 {
            schema_version: SCHEMA_VERSION,
            trace_id: self.trace_id,
            contract_ref: self.contract_ref,
            initial_state: self.initial_state,
            steps: self.steps,
            side_effect_claims: self.side_effects,
            terminal_state: self.last_state,
            coverage: CoverageClaim {
                trained_steps: trained,
                held_out_steps: 0,
            },
            digest: Digest::sha256(b""),
        }
        .bind_digest()
    }
}

/// Replace non-pure capability names. Digests stay; raw payloads never exist here.
#[must_use]
pub fn redact(mut trace: TraceV1) -> TraceV1 {
    for step in &mut trace.steps {
        if step.capability.as_str() != PURE_CAPABILITY {
            step.capability = BoundedText::new("redacted").expect("redacted fits");
        }
    }
    for claim in &mut trace.side_effect_claims {
        claim.capability = BoundedText::new("redacted").expect("redacted fits");
    }
    trace
        .bind_digest()
        .expect("redacted trace remains canonical")
}

/// Split the last step into a held-out partition. Training must remain non-empty.
pub fn partition_held_out(trace: &TraceV1) -> Result<(TraceV1, TraceV1), TraceError> {
    if trace.steps.len() < 2 {
        return Err(TraceError::Protocol(
            "held-out partition needs at least two steps",
        ));
    }
    let split = trace.steps.len() - 1;
    let mut train = trace.clone();
    train.steps.truncate(split);
    train.terminal_state = train
        .steps
        .last()
        .map(|step| step.after_state)
        .unwrap_or(train.initial_state);
    train.coverage = CoverageClaim {
        trained_steps: u32::try_from(train.steps.len()).expect("fits"),
        held_out_steps: 1,
    };
    let mut held = trace.clone();
    held.steps = vec![trace.steps[split].clone()];
    held.initial_state = held.steps[0].before_state;
    held.coverage = CoverageClaim {
        trained_steps: u32::try_from(train.steps.len()).expect("fits"),
        held_out_steps: 1,
    };
    Ok((train.bind_digest()?, held.bind_digest()?))
}

/// Replay report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayReport {
    /// Terminal state after replay.
    pub terminal_state: Digest,
    /// Trace digest that was replayed.
    pub trace_digest: Digest,
}

/// Deterministic replayer. Side effects require an explicit mock digest.
#[derive(Clone, Debug, Default)]
pub struct Replayer {
    mocks: BTreeMap<String, Digest>,
}

impl Replayer {
    /// Empty replayer. Only `pure` steps succeed.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mocks: BTreeMap::new(),
        }
    }

    /// Allow one capability if the recorded output digest matches `mock`.
    pub fn mock(&mut self, capability: &str, output_digest: Digest) -> Result<(), TraceError> {
        self.mocks.insert(
            BoundedText::new(capability)?.as_str().to_owned(),
            output_digest,
        );
        Ok(())
    }

    /// Replay. Default is side-effect denial.
    pub fn replay(&self, trace: &TraceV1, budget: Budget) -> Result<ReplayReport, TraceError> {
        if trace.schema_version != SCHEMA_VERSION {
            return Err(TraceError::Protocol("unsupported trace schema"));
        }
        if trace.steps.len() as u32 > budget.max_nodes() {
            return Err(TraceError::Budget);
        }
        if trace.compute_digest()? != trace.digest {
            return Err(TraceError::Protocol("trace digest mismatch"));
        }
        let mut state = trace.initial_state;
        let mut seen = BTreeSet::new();
        for step in &trace.steps {
            if !seen.insert(step.index) {
                return Err(TraceError::Protocol("duplicate step index"));
            }
            if step.before_state != state {
                return Err(TraceError::Protocol("step state is not contiguous"));
            }
            if step.capability.as_str() != PURE_CAPABILITY {
                match self.mocks.get(step.capability.as_str()) {
                    Some(expected) if *expected == step.output_digest => {}
                    Some(_) => {
                        return Err(TraceError::Protocol("mock output digest mismatch"));
                    }
                    None => {
                        return Err(TraceError::SideEffectDenied(
                            step.capability.as_str().to_owned(),
                        ));
                    }
                }
            }
            state = step.after_state;
        }
        if state != trace.terminal_state {
            return Err(TraceError::Protocol("terminal state mismatch"));
        }
        Ok(ReplayReport {
            terminal_state: state,
            trace_digest: trace.digest,
        })
    }
}

/// Sign a trace digest. The signature is not part of the trace digest.
pub fn sign_trace(trace: &TraceV1, signing: &SigningKey) -> Result<SignatureBlock, TraceError> {
    let digest_bytes = trace.digest.as_bytes();
    let signature = Ed25519SigningKey::from_bytes(signing.secret_bytes()).sign(&digest_bytes);
    Ok(SignatureBlock {
        algorithm: BoundedText::new("ed25519")?,
        key_id: BoundedText::new(&signing.key_id)?,
        value: hex::encode(signature.to_bytes()),
    })
}

/// Verify a trace certificate against a public key.
pub fn verify_trace(
    trace: &TraceV1,
    signature: &SignatureBlock,
    public: &[u8; 32],
) -> Result<(), TraceError> {
    if signature.algorithm.as_str() != "ed25519" {
        return Err(TraceError::Crypto("unsupported signature algorithm"));
    }
    if trace.compute_digest()? != trace.digest {
        return Err(TraceError::Protocol("trace digest mismatch"));
    }
    let bytes =
        hex::decode(&signature.value).map_err(|_| TraceError::Crypto("signature is not hex"))?;
    let bytes: [u8; 64] = bytes
        .try_into()
        .map_err(|_| TraceError::Crypto("signature is not 64 bytes"))?;
    let digest_bytes = trace.digest.as_bytes();
    VerifyingKey::from_bytes(public)
        .map_err(|_| TraceError::Crypto("public key is invalid"))?
        .verify(&digest_bytes, &Signature::from_bytes(&bytes))
        .map_err(|_| TraceError::Crypto("signature verification failed"))
}

/// Independent digest checker. Rebuilds the unsigned view instead of trusting `digest`.
pub fn independent_trace_digest(trace: &TraceV1) -> Result<Digest, TraceError> {
    let mut steps = trace.steps.clone();
    steps.sort_by_key(|step| step.index);
    let view = UnsignedTrace {
        schema_version: trace.schema_version,
        trace_id: trace.trace_id,
        contract_ref: &trace.contract_ref,
        initial_state: trace.initial_state,
        steps: &steps,
        side_effect_claims: &trace.side_effect_claims,
        terminal_state: trace.terminal_state,
        coverage: &trace.coverage,
    };
    Ok(Digest::sha256(&encode_canonical_cbor(&view)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> Budget {
        Budget::standard()
    }

    fn recorder() -> Recorder {
        Recorder::new("contract:v1", Digest::sha256(b"s0")).unwrap()
    }

    #[test]
    fn records_and_replays_pure_steps() {
        let mut rec = recorder();
        rec.record_step(
            Digest::sha256(b"in"),
            Digest::sha256(b"out"),
            Digest::sha256(b"s1"),
            1,
            vec![],
            PURE_CAPABILITY,
            budget(),
        )
        .unwrap();
        rec.record_step(
            Digest::sha256(b"in2"),
            Digest::sha256(b"out2"),
            Digest::sha256(b"s2"),
            2,
            vec![0],
            PURE_CAPABILITY,
            budget(),
        )
        .unwrap();
        let trace = rec.finish().unwrap();
        assert_eq!(independent_trace_digest(&trace).unwrap(), trace.digest);
        let report = Replayer::new().replay(&trace, budget()).unwrap();
        assert_eq!(report.terminal_state, Digest::sha256(b"s2"));
    }

    #[test]
    fn replay_denies_unmocked_side_effects() {
        let mut rec = recorder();
        rec.record_step(
            Digest::sha256(b"in"),
            Digest::sha256(b"net"),
            Digest::sha256(b"s1"),
            1,
            vec![],
            "network.write",
            budget(),
        )
        .unwrap();
        let trace = rec.finish().unwrap();
        assert!(matches!(
            Replayer::new().replay(&trace, budget()),
            Err(TraceError::SideEffectDenied(name)) if name == "network.write"
        ));
        let mut replayer = Replayer::new();
        replayer
            .mock("network.write", Digest::sha256(b"net"))
            .unwrap();
        replayer.replay(&trace, budget()).unwrap();
    }

    #[test]
    fn budget_exhaustion_is_not_success() {
        let mut rec = recorder();
        let tiny = Budget::new(1, 1, 1, 8, 10).unwrap();
        rec.record_step(
            Digest::sha256(b"in"),
            Digest::sha256(b"out"),
            Digest::sha256(b"s1"),
            1,
            vec![],
            PURE_CAPABILITY,
            tiny,
        )
        .unwrap();
        assert!(matches!(
            rec.record_step(
                Digest::sha256(b"in2"),
                Digest::sha256(b"out2"),
                Digest::sha256(b"s2"),
                2,
                vec![],
                PURE_CAPABILITY,
                tiny,
            ),
            Err(TraceError::Budget)
        ));
    }

    #[test]
    fn held_out_partition_is_deterministic() {
        let mut rec = recorder();
        rec.record_step(
            Digest::sha256(b"a"),
            Digest::sha256(b"b"),
            Digest::sha256(b"s1"),
            1,
            vec![],
            PURE_CAPABILITY,
            budget(),
        )
        .unwrap();
        rec.record_step(
            Digest::sha256(b"c"),
            Digest::sha256(b"d"),
            Digest::sha256(b"s2"),
            2,
            vec![0],
            PURE_CAPABILITY,
            budget(),
        )
        .unwrap();
        let trace = rec.finish().unwrap();
        let (train, held) = partition_held_out(&trace).unwrap();
        assert_eq!(train.steps.len(), 1);
        assert_eq!(held.steps.len(), 1);
        let (train2, held2) = partition_held_out(&trace).unwrap();
        assert_eq!(train.digest, train2.digest);
        assert_eq!(held.digest, held2.digest);
    }

    #[test]
    fn certificate_round_trip() {
        let mut rec = recorder();
        rec.record_step(
            Digest::sha256(b"in"),
            Digest::sha256(b"out"),
            Digest::sha256(b"s1"),
            1,
            vec![],
            PURE_CAPABILITY,
            budget(),
        )
        .unwrap();
        let trace = rec.finish().unwrap();
        let signing = SigningKey::from_bytes("k1", [3_u8; 32]);
        let sig = sign_trace(&trace, &signing).unwrap();
        verify_trace(&trace, &sig, &signing.public_bytes()).unwrap();
        let mut forged = sig.clone();
        forged.value = "00".repeat(64);
        assert!(verify_trace(&trace, &forged, &signing.public_bytes()).is_err());
    }
}
