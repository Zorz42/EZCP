//! What a run found, for programs: [`Task::run_report`](crate::Task::run_report) returns it and
//! `--json` prints it. Solutions, subtasks, generators and tests count from zero here.

use crate::Error;
pub use crate::outcome::{Outcome, Verdict};
use serde::Serialize;
use serde::ser::{SerializeStruct, Serializer};
use std::collections::BTreeMap;

/// Inputs and outputs in a report are cut to this many bytes.
const EXCERPT_BYTES: usize = 1000;

#[derive(Debug, Default)]
pub struct Report {
    pub seed: Option<u64>,
    pub subtasks: Vec<SubtaskInfo>,
    /// The official solution on every subtask, once the tests are finished.
    pub official: Vec<SubtaskResult>,
    pub partials: Vec<PartialReport>,
    pub mismatches: Vec<Mismatch>,
    /// Once the tests are finished, in file order.
    pub tests: Vec<TestInfo>,
    pub error: Option<Error>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubtaskInfo {
    pub name: String,
    pub points: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartialReport {
    pub name: String,
    /// The subtasks it is declared to pass, sorted.
    pub declared: Vec<usize>,
    /// On every subtask, once the tests are finished.
    pub results: Vec<SubtaskResult>,
}

#[derive(Debug, Clone, Default)]
pub struct SubtaskResult {
    pub verdicts: BTreeMap<Verdict, usize>,
    /// Not run after an earlier failure.
    pub skipped: usize,
    /// The largest over the tests it passed.
    pub time_ms: i32,
    pub memory_kb: u64,
    /// The first test it failed, counted within the subtask.
    pub first_failure: Option<usize>,
}

impl SubtaskResult {
    #[must_use]
    pub fn passed(&self) -> bool {
        // A subtask without tests passes nobody.
        self.skipped == 0 && self.verdicts.keys().eq([&Verdict::Ok])
    }
}

/// A partial solution doing other than declared.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Mismatch {
    /// It failed a test of a subtask it is declared to pass.
    Fails { solution: usize, subtask: usize, verdict: Verdict, test: FailedTest },
    /// No test of a subtask it is declared to fail breaks it.
    Passes { solution: usize, subtask: usize },
}

#[derive(Debug, Clone, Serialize)]
pub struct FailedTest {
    pub generator: usize,
    #[serde(serialize_with = "hex_seed")]
    pub seed: u64,
    pub input: String,
    pub expected: String,
    /// The solution's output, if it finished.
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestInfo {
    pub subtask: usize,
    pub generator: usize,
    #[serde(serialize_with = "hex_seed")]
    pub seed: u64,
    pub input_file: String,
    pub output_file: String,
}

/// At most [`EXCERPT_BYTES`] of `text`, and how long it was.
#[must_use]
pub fn excerpt(text: &str) -> String {
    if text.len() <= EXCERPT_BYTES {
        return text.to_owned();
    }
    let cut = (0..=EXCERPT_BYTES).rev().find(|&end| text.is_char_boundary(end)).unwrap_or(0);
    format!("{}... ({} bytes)", &text[..cut], text.len())
}

fn seed(seed: u64) -> String {
    format!("{seed:#018x}")
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde's signature")]
fn hex_seed<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&seed(*value))
}

impl Serialize for SubtaskResult {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut json = serializer.serialize_struct("SubtaskResult", 6)?;
        json.serialize_field("passed", &self.passed())?;
        json.serialize_field("verdicts", &self.verdicts)?;
        json.serialize_field("skipped", &self.skipped)?;
        json.serialize_field("time_ms", &self.time_ms)?;
        json.serialize_field("memory_kb", &self.memory_kb)?;
        json.serialize_field("first_failure", &self.first_failure)?;
        json.end()
    }
}

#[derive(Serialize)]
struct ErrorJson {
    kind: &'static str,
    message: String,
}

impl Serialize for Report {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let error = self.error.as_ref().map(|err| ErrorJson {
            kind: err.kind(),
            message: err.to_string(),
        });
        let mut json = serializer.serialize_struct("Report", 8)?;
        json.serialize_field("passed", &self.error.is_none())?;
        json.serialize_field("error", &error)?;
        json.serialize_field("seed", &self.seed.map(seed))?;
        json.serialize_field("subtasks", &self.subtasks)?;
        json.serialize_field("official", &self.official)?;
        json.serialize_field("partials", &self.partials)?;
        json.serialize_field("mismatches", &self.mismatches)?;
        json.serialize_field("tests", &self.tests)?;
        json.end()
    }
}

impl Report {
    /// The run's error, if it had one.
    pub fn into_result(self) -> crate::Result<()> {
        self.error.map_or(Ok(()), Err)
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        // Verdicts, the only map keys, serialize as strings, so this cannot fail.
        serde_json::to_string(self).unwrap_or_default()
    }
}
