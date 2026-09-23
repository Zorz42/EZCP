//! The one-line JSON a test file holds in [seed mode](crate::Mode::Seeds).
//!
//! The seed and hash are hexadecimal strings because JSON readers that parse
//! numbers as doubles would lose their low bits.

use crate::{Error, Result};
use serde_json::Value;

/// FNV-1a, a hash that, unlike `DefaultHasher`, never changes between builds.
#[must_use]
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

/// Which half of a test a stub stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The input file.
    Input,
    /// The output file, which is rebuilt by running the official solution.
    Output,
}

impl Part {
    /// The name used in a stub.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

/// Everything needed to rebuild one file of one test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stub {
    /// 0-based subtask index.
    pub subtask: usize,
    /// 0-based index of the generator within the subtask.
    pub generator: usize,
    /// The seed the generator is run with.
    pub seed: u64,
    /// Which file of the test this is.
    pub part: Part,
    /// [`stable_hash`] of the file, to detect generators that changed since.
    /// `None` skips the check.
    pub hash: Option<u64>,
}

fn index(object: &Value, key: &str) -> Result<usize> {
    object.get(key).and_then(Value::as_u64).and_then(|value| usize::try_from(value).ok()).ok_or_else(|| Error::InvalidStub {
        details: format!("\"{key}\" is missing or is not a non-negative integer"),
    })
}

/// Also accepts a plain number, for stubs written by hand.
fn hex_u64(value: &Value, what: &str) -> Result<u64> {
    match value {
        Value::String(text) => u64::from_str_radix(text.trim_start_matches("0x"), 16).map_err(|_ignored| Error::InvalidStub {
            details: format!("{what} is not a 64-bit hexadecimal number: \"{text}\""),
        }),
        Value::Number(number) => number.as_u64().ok_or_else(|| Error::InvalidStub {
            details: format!("{what} is not a non-negative integer"),
        }),
        _ => Err(Error::InvalidStub {
            details: format!("{what} is neither a string nor a number"),
        }),
    }
}

impl Stub {
    /// Renders the stub as a line of JSON. Nothing in it needs escaping.
    #[must_use]
    pub fn to_line(&self) -> String {
        let hash = self.hash.map_or_else(String::new, |hash| format!(",\"hash\":\"{hash:016x}\""));
        format!(
            "{{\"subtask\":{},\"generator\":{},\"seed\":\"{:016x}\",\"part\":\"{}\"{hash}}}\n",
            self.subtask,
            self.generator,
            self.seed,
            self.part.as_str()
        )
    }

    /// Parses a line written by [`Stub::to_line`], or one written by hand.
    pub fn parse(line: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(line).map_err(|err| Error::InvalidStub {
            details: format!("not a JSON object: {err}"),
        })?;

        let part = match value.get("part").and_then(Value::as_str) {
            Some("input") => Part::Input,
            Some("output") => Part::Output,
            Some(other) => {
                return Err(Error::InvalidStub {
                    details: format!("\"part\" is \"{other}\"; it has to be \"input\" or \"output\""),
                });
            }
            None => {
                return Err(Error::InvalidStub {
                    details: "\"part\" is missing; a stub stands for either the \"input\" or the \"output\" of a test".to_owned(),
                });
            }
        };

        let hash = value.get("hash").map(|hash| hex_u64(hash, "\"hash\"")).transpose()?;
        let seed = value.get("seed").ok_or_else(|| Error::InvalidStub {
            details: "\"seed\" is missing".to_owned(),
        })?;

        Ok(Self {
            subtask: index(&value, "subtask")?,
            generator: index(&value, "generator")?,
            seed: hex_u64(seed, "\"seed\"")?,
            part,
            hash,
        })
    }
}
