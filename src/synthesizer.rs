use std::collections::BTreeMap;

use crate::model::{Candidate, Error, FragmentContent, Grammar};

pub fn reconstruct(
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
            let mut matched = false;
            for next_state in &grammar.states {
                for output in &grammar.outputs {
                    examined = examined.checked_add(1).ok_or_else(|| {
                        Error::SearchExhausted("search-work counter overflow".into())
                    })?;
                    if examined > grammar.max_candidates {
                        return Err(Error::SearchExhausted("search budget exhausted".into()));
                    }
                    if next_state == &required.next_state && output == &required.output {
                        transitions.push(required.clone());
                        matched = true;
                        break;
                    }
                }
                if matched {
                    break;
                }
            }
            if !matched {
                return Err(Error::InsufficientEvidence(
                    "no grammar candidate satisfies a transition".into(),
                ));
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
