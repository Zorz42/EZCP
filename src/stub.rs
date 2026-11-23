//! The one-line JSON a test file holds in seed mode. Seed and hash are hex strings, since JSON readers
//! that parse numbers as doubles would lose their low bits.

use crate::Error;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Part {
    Input,
    /// Rebuilt by running the official solution.
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stub {
    pub subtask: usize,
    pub generator: usize,
    #[serde(serialize_with = "to_hex", deserialize_with = "from_hex")]
    pub seed: u64,
    pub part: Part,
    /// [`stable_hash`] of the file, to catch generators that changed since. `None` skips the check.
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "to_hex_option", deserialize_with = "from_hex_option")]
    pub hash: Option<u64>,
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde's signature")]
fn to_hex<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&format!("{value:016x}"))
}

#[allow(clippy::ref_option, reason = "serde's signature")]
fn to_hex_option<S: Serializer>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error> {
    value.map(|value| format!("{value:016x}")).serialize(serializer)
}

/// Also accepts a plain number, for stubs written by hand.
fn from_hex<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum HexOrNumber {
        Hex(String),
        Number(u64),
    }

    match HexOrNumber::deserialize(deserializer)? {
        HexOrNumber::Number(number) => Ok(number),
        HexOrNumber::Hex(text) => u64::from_str_radix(text.trim_start_matches("0x"), 16).map_err(|_ignored| D::Error::custom(format!("\"{text}\" is not a 64-bit hexadecimal number"))),
    }
}

fn from_hex_option<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    from_hex(deserializer).map(Some)
}

impl Stub {
    pub fn to_line(self) -> String {
        // Only numbers and hex strings, which always serialize.
        serde_json::to_string(&self).unwrap_or_default() + "\n"
    }

    pub fn parse(line: &str) -> crate::Result<Self> {
        serde_json::from_str(line).map_err(|err| Error::InvalidStub { details: err.to_string() })
    }
}
