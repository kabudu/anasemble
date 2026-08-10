//! Independent candidate interpreter.
//!
//! This module reparses candidate JSON and uses a nested map representation. It
//! does not invoke synthesizer evaluation logic.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::model::{Candidate, Error, FragmentContent, Grammar, TraceRole, Transition};

#[derive(Debug, Serialize)]
pub struct Coverage {
    pub mandatory_obligations: usize,
    pub passed_obligations: usize,
    pub held_out_traces: usize,
    pub passed_held_out_traces: usize,
    pub passed_negative_cases: usize,
    pub passed_metamorphic_properties: usize,
    pub uncovered_obligations: usize,
}

pub fn certify(
    candidate_json: &[u8],
    expected_component: &str,
    expected_interface_version: &str,
    contents: &[FragmentContent],
) -> Result<Coverage, Error> {
    let candidate = decode_candidate(candidate_json)?;
    candidate
        .grammar
        .validate()
        .map_err(|error| Error::CheckerRejected(error.to_string()))?;
    if candidate.component != expected_component
        || candidate.interface_version != expected_interface_version
    {
        return Err(Error::CheckerRejected(
            "candidate identity does not match the recovery target".into(),
        ));
    }
    let mut table: BTreeMap<String, BTreeMap<String, (String, String)>> = candidate
        .grammar
        .states
        .iter()
        .map(|state| (state.clone(), BTreeMap::new()))
        .collect();
    for transition in &candidate.transitions {
        transition
            .validate(&candidate.grammar)
            .map_err(|error| Error::CheckerRejected(error.to_string()))?;
        let state_table = table
            .get_mut(&transition.state)
            .ok_or_else(|| Error::CheckerRejected("candidate has an unknown state".into()))?;
        if state_table
            .insert(
                transition.input.clone(),
                (transition.next_state.clone(), transition.output.clone()),
            )
            .is_some()
        {
            return Err(Error::CheckerRejected(
                "candidate has duplicate transitions".into(),
            ));
        }
    }
    let expected = candidate.grammar.states.len() * candidate.grammar.inputs.len();
    if table.values().map(BTreeMap::len).sum::<usize>() != expected {
        return Err(Error::CheckerRejected(
            "candidate transition table is not total".into(),
        ));
    }

    let mut passed = 0;
    let mut trace_count = 0;
    let mut state_policy_count = 0;
    let mut negative_count = 0;
    let mut metamorphic_count = 0;
    for content in contents {
        match content {
            FragmentContent::Transition {
                state,
                input,
                next_state,
                output,
            } => {
                let expected_transition = Transition {
                    state: state.clone(),
                    input: input.clone(),
                    next_state: next_state.clone(),
                    output: output.clone(),
                };
                expected_transition
                    .validate(&candidate.grammar)
                    .map_err(|error| Error::CheckerRejected(error.to_string()))?;
                if table[state].get(input) != Some(&(next_state.clone(), output.clone())) {
                    return Err(Error::CheckerRejected(
                        "candidate violates a mandatory transition".into(),
                    ));
                }
                passed += 1;
            }
            FragmentContent::Trace {
                role: TraceRole::HeldOut,
                initial_state,
                inputs,
                outputs,
            } => {
                if inputs.len() != outputs.len() || !table.contains_key(initial_state) {
                    return Err(Error::CheckerRejected("held-out trace is malformed".into()));
                }
                let mut state = initial_state.clone();
                let mut observed = Vec::with_capacity(inputs.len());
                for input in inputs {
                    let (next_state, output) = table
                        .get(&state)
                        .and_then(|transitions| transitions.get(input))
                        .ok_or_else(|| {
                            Error::CheckerRejected("held-out trace uses an unknown input".into())
                        })?;
                    state.clone_from(next_state);
                    observed.push(output.clone());
                }
                if &observed != outputs {
                    return Err(Error::CheckerRejected(
                        "candidate violates a held-out trace".into(),
                    ));
                }
                trace_count += 1;
            }
            FragmentContent::Trace {
                role: TraceRole::Training,
                ..
            } => {}
            FragmentContent::NegativeCase {
                initial_state,
                inputs,
                forbidden_outputs,
            } => {
                if run_trace(&table, initial_state, inputs)? == *forbidden_outputs {
                    return Err(Error::CheckerRejected(
                        "candidate violates a mandatory negative case".into(),
                    ));
                }
                negative_count += 1;
            }
            FragmentContent::MetamorphicProperty {
                initial_state,
                input,
                repetitions,
            } => {
                if *repetitions < 2
                    || !check_idempotent(&table, initial_state, input, *repetitions)?
                {
                    return Err(Error::CheckerRejected(
                        "candidate violates a metamorphic property".into(),
                    ));
                }
                metamorphic_count += 1;
            }
            FragmentContent::StatePolicy {
                states,
                initial_state,
            } => {
                if states != &candidate.grammar.states
                    || initial_state != &candidate.grammar.initial_state
                {
                    return Err(Error::CheckerRejected(
                        "candidate grammar violates the state policy".into(),
                    ));
                }
                state_policy_count += 1;
            }
        }
    }
    if passed != expected {
        return Err(Error::CheckerRejected(
            "checker lacks a complete transition contract".into(),
        ));
    }
    if trace_count == 0 {
        return Err(Error::CheckerRejected(
            "checker requires a held-out trace".into(),
        ));
    }
    if state_policy_count != 1 {
        return Err(Error::CheckerRejected(
            "checker requires exactly one matching state policy".into(),
        ));
    }
    Ok(Coverage {
        mandatory_obligations: expected,
        passed_obligations: passed,
        held_out_traces: trace_count,
        passed_held_out_traces: trace_count,
        passed_negative_cases: negative_count,
        passed_metamorphic_properties: metamorphic_count,
        uncovered_obligations: 0,
    })
}

fn decode_candidate(input: &[u8]) -> Result<Candidate, Error> {
    let mut reader = Reader { input, offset: 0 };
    if reader.take(8)? != crate::checker_wire::magic() {
        return Err(Error::CheckerRejected("checker wire magic mismatch".into()));
    }
    let component = reader.string()?;
    let interface_version = reader.string()?;
    let version = reader.string()?;
    let inputs = reader.strings()?;
    let outputs = reader.strings()?;
    let states = reader.strings()?;
    let initial_state = reader.string()?;
    let max_candidates = u64::from_be_bytes(reader.take(8)?.try_into().expect("eight bytes"));
    let count = reader.length()?;
    if count > 256 {
        return Err(Error::CheckerRejected(
            "checker wire transition count exceeds the DSL bound".into(),
        ));
    }
    let mut transitions = Vec::with_capacity(count);
    for _ in 0..count {
        transitions.push(Transition {
            state: reader.string()?,
            input: reader.string()?,
            next_state: reader.string()?,
            output: reader.string()?,
        });
    }
    if reader.offset != input.len() {
        return Err(Error::CheckerRejected(
            "checker wire has trailing data".into(),
        ));
    }
    Ok(Candidate {
        component,
        interface_version,
        grammar: Grammar {
            version,
            inputs,
            outputs,
            states,
            initial_state,
            max_candidates,
        },
        transitions,
    })
}

struct Reader<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self
            .offset
            .checked_add(length)
            .filter(|end| *end <= self.input.len())
            .ok_or_else(|| Error::CheckerRejected("checker wire is truncated".into()))?;
        let value = &self.input[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    fn length(&mut self) -> Result<usize, Error> {
        let value = u32::from_be_bytes(self.take(4)?.try_into().expect("four bytes"));
        usize::try_from(value)
            .map_err(|_| Error::CheckerRejected("checker wire length overflow".into()))
    }

    fn string(&mut self) -> Result<String, Error> {
        let length = self.length()?;
        if length > 65_536 {
            return Err(Error::CheckerRejected(
                "checker wire string is too large".into(),
            ));
        }
        std::str::from_utf8(self.take(length)?)
            .map(str::to_owned)
            .map_err(|_| Error::CheckerRejected("checker wire string is not UTF-8".into()))
    }

    fn strings(&mut self) -> Result<Vec<String>, Error> {
        let count = self.length()?;
        if count > 65_536 {
            return Err(Error::CheckerRejected(
                "checker wire vector is too large".into(),
            ));
        }
        (0..count).map(|_| self.string()).collect()
    }
}

fn run_trace(
    table: &BTreeMap<String, BTreeMap<String, (String, String)>>,
    initial_state: &str,
    inputs: &[String],
) -> Result<Vec<String>, Error> {
    let mut state = initial_state.to_owned();
    let mut outputs = Vec::with_capacity(inputs.len());
    for input in inputs {
        let (next_state, output) = table
            .get(&state)
            .and_then(|transitions| transitions.get(input))
            .ok_or_else(|| Error::CheckerRejected("obligation uses an unknown symbol".into()))?;
        state.clone_from(next_state);
        outputs.push(output.clone());
    }
    Ok(outputs)
}

fn check_idempotent(
    table: &BTreeMap<String, BTreeMap<String, (String, String)>>,
    initial_state: &str,
    input: &str,
    repetitions: u32,
) -> Result<bool, Error> {
    let (stable_state, stable_output) = table
        .get(initial_state)
        .and_then(|items| items.get(input))
        .ok_or_else(|| Error::CheckerRejected("property uses an unknown symbol".into()))?;
    let mut state = stable_state;
    for _ in 1..repetitions {
        let (next_state, output) = table
            .get(state)
            .and_then(|items| items.get(input))
            .ok_or_else(|| Error::CheckerRejected("property uses an unknown symbol".into()))?;
        if next_state != stable_state || output != stable_output {
            return Ok(false);
        }
        state = next_state;
    }
    Ok(true)
}
