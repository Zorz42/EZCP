//! The one-line JSON a test file holds in seed mode. Seed and hash are hex strings, since JSON readers
//! that parse numbers as doubles would lose their low bits.

use crate::{Error, Result};
use serde_json::Value;

/// FNV-1a: unlike `DefaultHasher`, it never changes between builds.
pub fn stable_hash(data: &str) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET_BASIS;
    for byte in data.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Input,
    /// Rebuilt by running the official solution.
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stub {
    pub subtask: usize,
    pub generator: usize,
    pub seed: u64,
    pub part: Part,
    /// [`stable_hash`] of the file, to catch generators that changed since. `None` skips the check.
    pub hash: Option<u64>,
}

const fn invalid(details: String) -> Error {
    Error::InvalidStub { details }
}

fn index(object: &Value, key: &str) -> Result<usize> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| invalid(format!("\"{key}\" is missing or is not a non-negative integer")))
}

/// Also accepts a plain number, for stubs written by hand.
fn hex_u64(value: &Value, what: &str) -> Result<u64> {
    match value {
        Value::String(text) => u64::from_str_radix(text.trim_start_matches("0x"), 16).map_err(|_ignored| invalid(format!("{what} is not a 64-bit hexadecimal number: \"{text}\""))),
        Value::Number(number) => number.as_u64().ok_or_else(|| invalid(format!("{what} is not a non-negative integer"))),
        _ => Err(invalid(format!("{what} is neither a string nor a number"))),
    }
}

impl Stub {
    pub fn to_line(self) -> String {
        let hash = self.hash.map_or_else(String::new, |hash| format!(",\"hash\":\"{hash:016x}\""));
        let part = match self.part {
            Part::Input => "input",
            Part::Output => "output",
        };
        format!(
            "{{\"subtask\":{},\"generator\":{},\"seed\":\"{:016x}\",\"part\":\"{part}\"{hash}}}\n",
            self.subtask, self.generator, self.seed
        )
    }

    pub fn parse(line: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(line).map_err(|err| invalid(format!("not a JSON object: {err}")))?;
        let part = match value.get("part").and_then(Value::as_str) {
            Some("input") => Part::Input,
            Some("output") => Part::Output,
            Some(other) => return Err(invalid(format!("\"part\" is \"{other}\"; it has to be \"input\" or \"output\""))),
            None => return Err(invalid("\"part\" is missing; a stub stands for either the \"input\" or the \"output\" of a test".to_owned())),
        };
        let seed = value.get("seed").ok_or_else(|| invalid("\"seed\" is missing".to_owned()))?;

        Ok(Self {
            subtask: index(&value, "subtask")?,
            generator: index(&value, "generator")?,
            seed: hex_u64(seed, "\"seed\"")?,
            part,
            hash: value.get("hash").map(|hash| hex_u64(hash, "\"hash\"")).transpose()?,
        })
    }
}
