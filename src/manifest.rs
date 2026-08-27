//! The seed manifest: a description of a task's tests small enough to keep,
//! instead of the tests themselves.
//!
//! Every generated test is a generator plus the seed it was run with, so a few
//! dozen bytes stand in for a file that can be megabytes. A judge stores the
//! manifest, and asks [the server](crate::serve) for the actual bytes of a test
//! when it needs them.
//!
//! The file is JSON so that a judge written in anything can read it. Seeds are
//! written as hexadecimal strings rather than numbers: a seed uses all 64 bits,
//! and a JSON number would silently lose the low ones in any reader that parses
//! numbers as doubles, JavaScript above all.

use crate::task::path_str;
use crate::{Error, Result};
use serde_json::{Map, Value, json};
use std::path::Path;

/// The name written into the `format` field, so a reader can tell this file from
/// any other JSON it might be handed.
const FORMAT_TAG: &str = "ezcp-seeds";

/// The manifest layout version.
///
/// A reader that does not know a version must refuse the file rather than guess
/// at it, so this is bumped whenever a field changes meaning.
const FORMAT_VERSION: u64 = 1;

/// A 64-bit hash that does not change between Rust releases.
///
/// `DefaultHasher` explicitly makes no such promise, and a hash recorded in a
/// manifest is compared against one computed by a different build, possibly
/// years later. This is FNV-1a, which is fixed by its specification.
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

/// One test, recorded as the recipe that produces it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestTest {
    /// Position of this test within its subtask.
    pub index_in_subtask: usize,
    /// Position of this test among all of the task's tests.
    pub global_index: usize,
    /// Which of the subtask's generators produced it.
    pub generator: usize,
    /// The seed that generator was run with.
    pub seed: u64,
    /// The name the input would have as a file, so a judge can keep the naming
    /// of a normal run.
    pub input_file: String,
    /// The name the output would have as a file.
    pub output_file: String,
    /// Hash of the generated input, to detect a generator that has changed since
    /// the manifest was written.
    pub input_hash: u64,
    /// Hash of the official solution's output for that input.
    pub output_hash: u64,
}

/// One subtask's worth of tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestSubtask {
    pub index: usize,
    pub points: i32,
    pub name: String,
    pub tests: Vec<ManifestTest>,
}

/// Everything needed to rebuild a task's tests from seeds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub task: String,
    /// The master seed the run was started from. Re-running the whole task with
    /// it reproduces this exact manifest.
    pub seed: u64,
    /// Whether generated tests had their whitespace normalised, which changes the
    /// bytes and therefore has to be recorded.
    pub trim_whitespace: bool,
    pub time_limit: i32,
    pub subtasks: Vec<ManifestSubtask>,
}

/// Reads a required field, naming it if it is missing or of the wrong type.
fn field<'json>(object: &'json Map<String, Value>, key: &str) -> Result<&'json Value> {
    object.get(key).ok_or_else(|| Error::InvalidManifest {
        details: format!("missing field \"{key}\""),
    })
}

fn as_object<'json>(value: &'json Value, what: &str) -> Result<&'json Map<String, Value>> {
    value.as_object().ok_or_else(|| Error::InvalidManifest {
        details: format!("{what} is not an object"),
    })
}

fn as_usize(value: &Value, what: &str) -> Result<usize> {
    value.as_u64().and_then(|number| usize::try_from(number).ok()).ok_or_else(|| Error::InvalidManifest {
        details: format!("{what} is not a non-negative integer"),
    })
}

fn as_str<'json>(value: &'json Value, what: &str) -> Result<&'json str> {
    value.as_str().ok_or_else(|| Error::InvalidManifest {
        details: format!("{what} is not a string"),
    })
}

/// Parses one of the hexadecimal 64-bit values the manifest stores as strings.
fn as_hex_u64(value: &Value, what: &str) -> Result<u64> {
    let text = as_str(value, what)?;
    u64::from_str_radix(text, 16).map_err(|_ignored| Error::InvalidManifest {
        details: format!("{what} is not a 64-bit hexadecimal number: \"{text}\""),
    })
}

/// Formats a 64-bit value the way the manifest stores it.
fn hex(value: u64) -> String {
    format!("{value:016x}")
}

impl ManifestTest {
    fn to_json(&self) -> Value {
        json!({
            "index": self.index_in_subtask,
            "global_index": self.global_index,
            "generator": self.generator,
            "seed": hex(self.seed),
            "input_file": self.input_file,
            "output_file": self.output_file,
            "input_hash": hex(self.input_hash),
            "output_hash": hex(self.output_hash),
        })
    }

    fn from_json(value: &Value) -> Result<Self> {
        let object = as_object(value, "a test")?;
        Ok(Self {
            index_in_subtask: as_usize(field(object, "index")?, "a test's index")?,
            global_index: as_usize(field(object, "global_index")?, "a test's global index")?,
            generator: as_usize(field(object, "generator")?, "a test's generator")?,
            seed: as_hex_u64(field(object, "seed")?, "a test's seed")?,
            input_file: as_str(field(object, "input_file")?, "a test's input file name")?.to_owned(),
            output_file: as_str(field(object, "output_file")?, "a test's output file name")?.to_owned(),
            input_hash: as_hex_u64(field(object, "input_hash")?, "a test's input hash")?,
            output_hash: as_hex_u64(field(object, "output_hash")?, "a test's output hash")?,
        })
    }
}

impl ManifestSubtask {
    fn to_json(&self) -> Value {
        json!({
            "index": self.index,
            "points": self.points,
            "name": self.name,
            "tests": self.tests.iter().map(ManifestTest::to_json).collect::<Vec<_>>(),
        })
    }

    fn from_json(value: &Value) -> Result<Self> {
        let object = as_object(value, "a subtask")?;
        let tests = field(object, "tests")?.as_array().ok_or_else(|| Error::InvalidManifest {
            details: "a subtask's tests are not an array".to_owned(),
        })?;

        Ok(Self {
            index: as_usize(field(object, "index")?, "a subtask's index")?,
            points: field(object, "points")?.as_i64().unwrap_or(0) as i32,
            name: as_str(field(object, "name")?, "a subtask's name")?.to_owned(),
            tests: tests.iter().map(ManifestTest::from_json).collect::<Result<Vec<_>>>()?,
        })
    }
}

impl Manifest {
    /// Renders the manifest as pretty-printed JSON.
    ///
    /// Pretty-printed because a manifest is read by people at least as often as
    /// by programs when a task's test data is being argued about.
    #[must_use]
    pub fn to_json_string(&self) -> String {
        let value = json!({
            "format": FORMAT_TAG,
            "version": FORMAT_VERSION,
            "task": self.task,
            "seed": hex(self.seed),
            "trim_whitespace": self.trim_whitespace,
            "time_limit": self.time_limit,
            "subtasks": self.subtasks.iter().map(ManifestSubtask::to_json).collect::<Vec<_>>(),
        });
        serde_json::to_string_pretty(&value).unwrap_or_else(|_ignored| String::new())
    }

    /// Parses a manifest.
    pub fn from_json_string(text: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(text).map_err(|err| Error::InvalidManifest { details: err.to_string() })?;
        let object = as_object(&value, "the manifest")?;

        let format = as_str(field(object, "format")?, "the format tag")?;
        if format != FORMAT_TAG {
            return Err(Error::InvalidManifest {
                details: format!("this is not an EZCP seed manifest (its format is \"{format}\")"),
            });
        }

        let version = field(object, "version")?.as_u64().unwrap_or(0);
        if version != FORMAT_VERSION {
            return Err(Error::InvalidManifest {
                details: format!("manifest version {version} was written by a different version of EZCP; this one reads version {FORMAT_VERSION}"),
            });
        }

        let subtasks = field(object, "subtasks")?.as_array().ok_or_else(|| Error::InvalidManifest {
            details: "the subtasks are not an array".to_owned(),
        })?;

        Ok(Self {
            task: as_str(field(object, "task")?, "the task name")?.to_owned(),
            seed: as_hex_u64(field(object, "seed")?, "the master seed")?,
            trim_whitespace: field(object, "trim_whitespace")?.as_bool().unwrap_or(true),
            time_limit: field(object, "time_limit")?.as_i64().unwrap_or(0) as i32,
            subtasks: subtasks.iter().map(ManifestSubtask::from_json).collect::<Result<Vec<_>>>()?,
        })
    }

    /// Writes the manifest to `path`.
    pub fn write(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_json_string()).map_err(|err| Error::IOError { err, file: path_str(path) })
    }

    /// Reads a manifest from `path`.
    pub fn read(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|err| Error::IOError { err, file: path_str(path) })?;
        Self::from_json_string(&text).map_err(|err| match err {
            // The parse errors do not know which file they came from, and a
            // manifest is usually one of several files a judge is juggling.
            Error::InvalidManifest { details } => Error::InvalidManifest {
                details: format!("{}: {details}", path_str(path)),
            },
            other => other,
        })
    }

    /// Finds a test by its subtask and its position within it.
    #[must_use]
    pub fn find_test(&self, subtask: usize, test: usize) -> Option<&ManifestTest> {
        self.subtasks.iter().find(|candidate| candidate.index == subtask)?.tests.get(test)
    }

    /// The number of tests in the whole manifest.
    #[must_use]
    pub fn num_tests(&self) -> usize {
        self.subtasks.iter().map(|subtask| subtask.tests.len()).sum()
    }
}
