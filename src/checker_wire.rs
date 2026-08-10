use crate::model::{Candidate, Error};

const MAGIC: &[u8; 8] = b"ANCKM1\0\0";

pub fn encode_candidate(candidate: &Candidate) -> Result<Vec<u8>, Error> {
    let mut output = MAGIC.to_vec();
    push_string(&mut output, &candidate.component)?;
    push_string(&mut output, &candidate.interface_version)?;
    push_string(&mut output, &candidate.grammar.version)?;
    push_strings(&mut output, &candidate.grammar.inputs)?;
    push_strings(&mut output, &candidate.grammar.outputs)?;
    push_strings(&mut output, &candidate.grammar.states)?;
    push_string(&mut output, &candidate.grammar.initial_state)?;
    output.extend_from_slice(&candidate.grammar.max_candidates.to_be_bytes());
    push_len(&mut output, candidate.transitions.len())?;
    for transition in &candidate.transitions {
        push_string(&mut output, &transition.state)?;
        push_string(&mut output, &transition.input)?;
        push_string(&mut output, &transition.next_state)?;
        push_string(&mut output, &transition.output)?;
    }
    Ok(output)
}

fn push_strings(output: &mut Vec<u8>, values: &[String]) -> Result<(), Error> {
    push_len(output, values.len())?;
    for value in values {
        push_string(output, value)?;
    }
    Ok(())
}

fn push_string(output: &mut Vec<u8>, value: &str) -> Result<(), Error> {
    push_len(output, value.len())?;
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn push_len(output: &mut Vec<u8>, value: usize) -> Result<(), Error> {
    let value = u32::try_from(value)
        .map_err(|_| Error::CheckerRejected("checker wire value is too large".into()))?;
    output.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

pub(crate) const fn magic() -> &'static [u8; 8] {
    MAGIC
}
