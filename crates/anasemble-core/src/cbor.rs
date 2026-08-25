//! Canonical CBOR (RFC 8949 definite-length, sorted map keys via JSON Map).

use serde::Serialize;
use serde_json::Value;

use crate::CoreError;

/// Maximum nesting depth for canonical CBOR encoding and decoding.
pub const MAX_CBOR_DEPTH: usize = 16;

/// Encode `value` as canonical CBOR.
///
/// The value is first converted to JSON, then to CBOR. Map keys are the JSON
/// object's sorted keys. This is the signing representation for generic envelopes.
pub fn encode_canonical_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, CoreError> {
    let tree = serde_json::to_value(value)?;
    let mut out = Vec::new();
    encode_value(&mut out, &tree, 0)?;
    if out.len() > crate::MAX_FRAME_BYTES {
        return Err(CoreError::Bound);
    }
    Ok(out)
}

fn encode_value(out: &mut Vec<u8>, value: &Value, depth: usize) -> Result<(), CoreError> {
    if depth > MAX_CBOR_DEPTH {
        return Err(CoreError::Cbor("CBOR nesting exceeds depth bound"));
    }
    match value {
        Value::Null => out.push(0xf6),
        Value::Bool(false) => out.push(0xf4),
        Value::Bool(true) => out.push(0xf5),
        Value::Number(number) => encode_number(out, number)?,
        Value::String(text) => encode_text(out, text),
        Value::Array(items) => {
            encode_len(out, 4, items.len() as u64);
            for item in items {
                encode_value(out, item, depth + 1)?;
            }
        }
        Value::Object(map) => {
            encode_len(out, 5, map.len() as u64);
            for (key, item) in map {
                encode_text(out, key);
                encode_value(out, item, depth + 1)?;
            }
        }
    }
    Ok(())
}

fn encode_number(out: &mut Vec<u8>, number: &serde_json::Number) -> Result<(), CoreError> {
    if let Some(value) = number.as_u64() {
        encode_len(out, 0, value);
        return Ok(());
    }
    if let Some(value) = number.as_i64() {
        if value >= 0 {
            encode_len(out, 0, value as u64);
        } else {
            encode_len(out, 1, value.unsigned_abs() - 1);
        }
        return Ok(());
    }
    let float = number
        .as_f64()
        .ok_or(CoreError::Cbor("JSON number is not a finite CBOR float"))?;
    if !float.is_finite() {
        return Err(CoreError::Cbor("JSON number is not a finite CBOR float"));
    }
    out.push(0xfb);
    out.extend_from_slice(&float.to_be_bytes());
    Ok(())
}

fn encode_text(out: &mut Vec<u8>, text: &str) {
    encode_len(out, 3, text.len() as u64);
    out.extend_from_slice(text.as_bytes());
}

fn encode_len(out: &mut Vec<u8>, major: u8, len: u64) {
    let major = major << 5;
    if len < 24 {
        out.push(major | len as u8);
    } else if len <= u8::MAX as u64 {
        out.push(major | 24);
        out.push(len as u8);
    } else if len <= u16::MAX as u64 {
        out.push(major | 25);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    } else if len <= u32::MAX as u64 {
        out.push(major | 26);
        out.extend_from_slice(&(len as u32).to_be_bytes());
    } else {
        out.push(major | 27);
        out.extend_from_slice(&len.to_be_bytes());
    }
}

/// Decode a canonical CBOR value for mutation tests. Not a general decoder.
pub fn decode_to_json(bytes: &[u8]) -> Result<Value, CoreError> {
    let (value, rest) = decode_value(bytes, 0)?;
    if !rest.is_empty() {
        return Err(CoreError::Cbor("CBOR input has trailing bytes"));
    }
    Ok(value)
}

fn decode_value(bytes: &[u8], depth: usize) -> Result<(Value, &[u8]), CoreError> {
    if depth > MAX_CBOR_DEPTH {
        return Err(CoreError::Cbor("CBOR nesting exceeds depth bound"));
    }
    let (first, rest) = bytes
        .split_first()
        .ok_or(CoreError::Cbor("truncated CBOR"))?;
    match first {
        0xf4 => Ok((Value::Bool(false), rest)),
        0xf5 => Ok((Value::Bool(true), rest)),
        0xf6 => Ok((Value::Null, rest)),
        0xfb => {
            if rest.len() < 8 {
                return Err(CoreError::Cbor("truncated CBOR float"));
            }
            let mut bits = [0_u8; 8];
            bits.copy_from_slice(&rest[..8]);
            let float = f64::from_be_bytes(bits);
            Ok((
                Value::Number(
                    serde_json::Number::from_f64(float)
                        .ok_or(CoreError::Cbor("non-finite CBOR float"))?,
                ),
                &rest[8..],
            ))
        }
        b if *b >> 5 == 0 => {
            let (len, rest) = take_len(*b, rest)?;
            Ok((Value::Number(len.into()), rest))
        }
        b if *b >> 5 == 1 => {
            let (n, rest) = take_len(*b, rest)?;
            let value = -1_i64
                .checked_sub(
                    n.try_into()
                        .map_err(|_| CoreError::Cbor("negative out of range"))?,
                )
                .ok_or(CoreError::Cbor("negative out of range"))?;
            Ok((Value::Number(value.into()), rest))
        }
        b if *b >> 5 == 3 => {
            let (len, rest) = take_len(*b, rest)?;
            let len = usize::try_from(len).map_err(|_| CoreError::Cbor("text too large"))?;
            if rest.len() < len {
                return Err(CoreError::Cbor("truncated CBOR text"));
            }
            let text = std::str::from_utf8(&rest[..len])
                .map_err(|_| CoreError::Cbor("CBOR text is not UTF-8"))?;
            Ok((Value::String(text.to_owned()), &rest[len..]))
        }
        b if *b >> 5 == 4 => {
            let (len, mut rest) = take_len(*b, rest)?;
            let mut items = Vec::new();
            for _ in 0..len {
                let (item, next) = decode_value(rest, depth + 1)?;
                items.push(item);
                rest = next;
            }
            Ok((Value::Array(items), rest))
        }
        b if *b >> 5 == 5 => {
            let (len, mut rest) = take_len(*b, rest)?;
            let mut map = serde_json::Map::new();
            let mut last_key: Option<String> = None;
            for _ in 0..len {
                let (key, next) = decode_value(rest, depth + 1)?;
                let key = key
                    .as_str()
                    .ok_or(CoreError::Cbor("CBOR map key is not text"))?
                    .to_owned();
                if last_key
                    .as_ref()
                    .is_some_and(|previous| key.as_str() <= previous.as_str())
                {
                    return Err(CoreError::Cbor("CBOR map keys are not strictly sorted"));
                }
                last_key = Some(key.clone());
                let (item, next) = decode_value(next, depth + 1)?;
                map.insert(key, item);
                rest = next;
            }
            Ok((Value::Object(map), rest))
        }
        _ => Err(CoreError::Cbor("unsupported CBOR type")),
    }
}

fn take_len(first: u8, rest: &[u8]) -> Result<(u64, &[u8]), CoreError> {
    match first & 0x1f {
        n if n < 24 => Ok((u64::from(n), rest)),
        24 => {
            let (b, rest) = rest
                .split_first()
                .ok_or(CoreError::Cbor("truncated CBOR"))?;
            Ok((u64::from(*b), rest))
        }
        25 => {
            if rest.len() < 2 {
                return Err(CoreError::Cbor("truncated CBOR"));
            }
            Ok((
                u64::from(u16::from_be_bytes([rest[0], rest[1]])),
                &rest[2..],
            ))
        }
        26 => {
            if rest.len() < 4 {
                return Err(CoreError::Cbor("truncated CBOR"));
            }
            let mut bits = [0_u8; 4];
            bits.copy_from_slice(&rest[..4]);
            Ok((u64::from(u32::from_be_bytes(bits)), &rest[4..]))
        }
        27 => {
            if rest.len() < 8 {
                return Err(CoreError::Cbor("truncated CBOR"));
            }
            let mut bits = [0_u8; 8];
            bits.copy_from_slice(&rest[..8]);
            Ok((u64::from_be_bytes(bits), &rest[8..]))
        }
        _ => Err(CoreError::Cbor("indefinite CBOR is refused")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_map_is_a0() {
        assert_eq!(encode_canonical_cbor(&json!({})).unwrap(), vec![0xa0]);
    }

    #[test]
    fn sorted_keys_and_small_int() {
        let encoded = encode_canonical_cbor(&json!({"a": 1})).unwrap();
        assert_eq!(encoded, vec![0xa1, 0x61, b'a', 0x01]);
    }

    #[test]
    fn round_trip_object() {
        let value = json!({"z": [true, null], "a": "ok"});
        let encoded = encode_canonical_cbor(&value).unwrap();
        assert_eq!(decode_to_json(&encoded).unwrap(), value);
    }

    #[test]
    fn refuses_indefinite_break() {
        assert!(decode_to_json(&[0x9f, 0xff]).is_err());
    }
}
