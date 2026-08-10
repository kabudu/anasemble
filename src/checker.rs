//! Independent candidate interpreter.
//!
//! This module reparses candidate JSON and uses a nested map representation. It
//! does not invoke synthesizer evaluation logic.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::model::{Candidate, Error, FragmentContent, Transition};

#[derive(Debug, Serialize)]
pub struct Coverage {
    pub mandatory_obligations: usize,
    pub passed_obligations: usize,
    pub held_out_traces: usize,
    pub passed_held_out_traces: usize,
    pub uncovered_obligations: usize,
}

pub fn certify(
    candidate_json: &[u8],
    expected_component: &str,
    expected_interface_version: &str,
    contents: &[FragmentContent],
) -> Result<Coverage, Error> {
    let candidate: Candidate = serde_json::from_slice(candidate_json)
        .map_err(|error| Error::CheckerRejected(format!("candidate parse failed: {error}")))?;
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
        uncovered_obligations: 0,
    })
}
