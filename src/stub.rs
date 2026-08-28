//! What a test file holds in [seed mode](crate::Mode::Seeds): the recipe for a
//! test instead of the test itself.
//!
//! A generated test is a generator plus the seed it was run with, so a stub of a
//! few dozen bytes stands in for a file that can be megabytes. Seed mode writes a
//! whole test set of them, under the file names a normal run would have used, and
//! [the server](crate::serve) turns one back into the bytes it stands for:
//!
//! ```text
//! $ ./task --serve < tests/test.01.001.in > test.in
//! ```
//!
//! A stub is one line of JSON, so a judge written in anything can read it, and it
//! is self-contained: nothing else has to be kept alongside it. The seed and the
//! hash are hexadecimal strings rather than numbers, because both use all 64 bits
//! and a JSON number would silently lose the low ones in any reader that parses
//! numbers as doubles, JavaScript above all.

use crate::{Error, Result};
use serde_json::Value;

/// A 64-bit hash that does not change between Rust releases.
///
/// `DefaultHasher` explicitly makes no such promise, and a hash written into a
/// stub is compared against one computed by a different build, possibly years
/// later. This is FNV-1a, which is fixed by its specification.
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
    /// The test's input, as the `.in` file of a normal run holds it.
    Input,
    /// The official solution's output for that input, as the `.out` file holds
    /// it. Producing it means running the solution.
    Output,
}

impl Part {
    /// The word a stub writes for this half.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

/// Everything needed to rebuild one half of one test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stub {
    /// Which subtask the test belongs to, counted from zero.
    pub subtask: usize,
    /// Which of that subtask's generators produced it.
    pub generator: usize,
    /// The seed that generator was run with.
    pub seed: u64,
    /// Which half of the test this stub stands for.
    pub part: Part,
    /// Hash of the bytes it stands for, which is what catches a generator that
    /// has changed since the stub was written.
    ///
    /// `None` in a request written by hand, which the server then has to take on
    /// trust.
    pub hash: Option<u64>,
}

/// Reads a required index field.
fn index(object: &Value, key: &str) -> Result<usize> {
    object.get(key).and_then(Value::as_u64).and_then(|value| usize::try_from(value).ok()).ok_or_else(|| Error::InvalidStub {
        details: format!("\"{key}\" is missing or is not a non-negative integer"),
    })
}

/// Reads one of the 64-bit values a stub stores as a hexadecimal string.
///
/// A plain number is accepted too, for a request written by hand where the value
/// is small enough that nothing can be lost.
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
    /// Renders the stub as the single line a test file holds.
    ///
    /// Written by hand rather than through a serialiser to keep the fields in
    /// reading order; every value is a number or a fixed word, so there is
    /// nothing here that could need escaping.
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

    /// Parses a stub, which is also what a request to the server looks like.
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
