use std::collections::BTreeMap;

use crate::model::{Candidate, Error, FragmentContent, Grammar, TraceRole, Transition};

pub fn reconstruct(
    component: &str,
    interface_version: &str,
    grammar: &Grammar,
    contents: &[FragmentContent],
) -> Result<(Candidate, u64), Error> {
    match grammar.version.as_str() {
        "fsm-v0" => reconstruct_v0(component, interface_version, grammar, contents),
        "fsm-v1" => reconstruct_v1(component, interface_version, grammar, contents),
        _ => Err(Error::InvalidRegistry("unsupported grammar version".into())),
    }
}

fn reconstruct_v1(
    component: &str,
    interface_version: &str,
    grammar: &Grammar,
    contents: &[FragmentContent],
) -> Result<(Candidate, u64), Error> {
    let cells: Vec<_> = grammar
        .states
        .iter()
        .flat_map(|state| grammar.inputs.iter().map(move |input| (state, input)))
        .collect();
    let choices: Vec<_> = grammar
        .states
        .iter()
        .flat_map(|state| grammar.outputs.iter().map(move |output| (state, output)))
        .collect();
    let mut assignment = Vec::with_capacity(cells.len());
    let mut solutions = Vec::new();
    let mut examined = 0_u64;
    enumerate(
        0,
        &cells,
        &choices,
        &mut assignment,
        grammar,
        contents,
        &mut examined,
        &mut solutions,
    )?;
    match solutions.len() {
        0 => Err(Error::ContradictoryEvidence(
            "no candidate satisfies the normalized evidence".into(),
        )),
        1 => Ok((
            Candidate {
                component: component.into(),
                interface_version: interface_version.into(),
                grammar: grammar.clone(),
                transitions: solutions.pop().expect("one solution exists"),
            },
            examined,
        )),
        _ => Err(Error::InsufficientEvidence(
            "evidence admits multiple candidates".into(),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn enumerate(
    index: usize,
    cells: &[(&String, &String)],
    choices: &[(&String, &String)],
    assignment: &mut Vec<Transition>,
    grammar: &Grammar,
    contents: &[FragmentContent],
    examined: &mut u64,
    solutions: &mut Vec<Vec<Transition>>,
) -> Result<(), Error> {
    if solutions.len() > 1 {
        return Ok(());
    }
    if index == cells.len() {
        *examined = examined
            .checked_add(1)
            .ok_or_else(|| Error::SearchExhausted("candidate counter overflow".into()))?;
        if *examined > grammar.max_candidates {
            return Err(Error::SearchExhausted("search budget exhausted".into()));
        }
        if satisfies(assignment, grammar, contents)? {
            solutions.push(assignment.clone());
        }
        return Ok(());
    }
    let (state, input) = cells[index];
    for (next_state, output) in choices {
        assignment.push(Transition {
            state: (*state).clone(),
            input: (*input).clone(),
            next_state: (*next_state).clone(),
            output: (*output).clone(),
        });
        enumerate(
            index + 1,
            cells,
            choices,
            assignment,
            grammar,
            contents,
            examined,
            solutions,
        )?;
        assignment.pop();
        if solutions.len() > 1 {
            break;
        }
    }
    Ok(())
}

fn satisfies(
    transitions: &[Transition],
    grammar: &Grammar,
    contents: &[FragmentContent],
) -> Result<bool, Error> {
    let table: BTreeMap<_, _> = transitions
        .iter()
        .map(|item| {
            (
                (item.state.as_str(), item.input.as_str()),
                (item.next_state.as_str(), item.output.as_str()),
            )
        })
        .collect();
    for content in contents {
        match content {
            FragmentContent::Transition {
                state,
                input,
                next_state,
                output,
            } => {
                if table.get(&(state.as_str(), input.as_str()))
                    != Some(&(next_state.as_str(), output.as_str()))
                {
                    return Ok(false);
                }
            }
            FragmentContent::Trace {
                role: TraceRole::Training,
                initial_state,
                inputs,
                outputs,
            } => {
                if execute(&table, initial_state, inputs)? != *outputs {
                    return Ok(false);
                }
            }
            FragmentContent::NegativeCase {
                initial_state,
                inputs,
                forbidden_outputs,
            } => {
                if execute(&table, initial_state, inputs)? == *forbidden_outputs {
                    return Ok(false);
                }
            }
            FragmentContent::MetamorphicProperty {
                initial_state,
                input,
                repetitions,
            } => {
                if *repetitions < 2 || !idempotent(&table, initial_state, input, *repetitions)? {
                    return Ok(false);
                }
            }
            FragmentContent::StatePolicy {
                states,
                initial_state,
            } => {
                if states != &grammar.states || initial_state != &grammar.initial_state {
                    return Ok(false);
                }
            }
            FragmentContent::Trace {
                role: TraceRole::HeldOut,
                ..
            } => {}
        }
    }
    Ok(true)
}

fn execute(
    table: &BTreeMap<(&str, &str), (&str, &str)>,
    initial_state: &str,
    inputs: &[String],
) -> Result<Vec<String>, Error> {
    let mut state = initial_state;
    let mut outputs = Vec::with_capacity(inputs.len());
    for input in inputs {
        let (next_state, output) = table
            .get(&(state, input.as_str()))
            .ok_or_else(|| Error::InvalidEvidence("trace uses an unknown symbol".into()))?;
        state = next_state;
        outputs.push((*output).to_owned());
    }
    Ok(outputs)
}

fn idempotent(
    table: &BTreeMap<(&str, &str), (&str, &str)>,
    initial_state: &str,
    input: &str,
    repetitions: u32,
) -> Result<bool, Error> {
    let (stable_state, stable_output) = table
        .get(&(initial_state, input))
        .ok_or_else(|| Error::InvalidEvidence("property uses an unknown symbol".into()))?;
    let mut state = *stable_state;
    for _ in 1..repetitions {
        let (next_state, output) = table
            .get(&(state, input))
            .ok_or_else(|| Error::InvalidEvidence("property uses an unknown symbol".into()))?;
        if next_state != stable_state || output != stable_output {
            return Ok(false);
        }
        state = next_state;
    }
    Ok(true)
}

fn reconstruct_v0(
    component: &str,
    interface_version: &str,
    grammar: &Grammar,
    contents: &[FragmentContent],
) -> Result<(Candidate, u64), Error> {
    let mut obligations = BTreeMap::new();
    for transition in contents.iter().filter_map(FragmentContent::transition) {
        transition.validate(grammar)?;
        let key = (transition.state.clone(), transition.input.clone());
        if let Some(previous) = obligations.insert(key, transition.clone())
            && previous != transition
        {
            return Err(Error::ContradictoryEvidence(
                "contradictory transition obligations".into(),
            ));
        }
    }
    let expected = grammar.states.len() * grammar.inputs.len();
    if obligations.len() != expected {
        return Err(Error::InsufficientEvidence(
            "transition contract is not behaviorally complete".into(),
        ));
    }
    let mut examined = 0_u64;
    let mut transitions = Vec::with_capacity(expected);
    for state in &grammar.states {
        for input in &grammar.inputs {
            let required = &obligations[&(state.clone(), input.clone())];
            for next_state in &grammar.states {
                for output in &grammar.outputs {
                    examined += 1;
                    if examined > grammar.max_candidates {
                        return Err(Error::SearchExhausted("search budget exhausted".into()));
                    }
                    if next_state == &required.next_state && output == &required.output {
                        transitions.push(required.clone());
                    }
                }
            }
        }
    }
    Ok((
        Candidate {
            component: component.into(),
            interface_version: interface_version.into(),
            grammar: grammar.clone(),
            transitions,
        },
        examined,
    ))
}
