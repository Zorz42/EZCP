//! What a run found, for programs: [`Task::run_report`](crate::Task::run_report) returns it and
//! `--json` prints it. Solutions, subtasks, generators and tests count from zero here.

use crate::Error;
pub use crate::outcome::{Outcome, Verdict};
use serde_json::{Value, json};
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

#[derive(Debug, Clone)]
pub struct SubtaskInfo {
    pub name: String,
    pub points: i32,
}

#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub enum Mismatch {
    /// It failed a test of a subtask it is declared to pass.
    Fails { solution: usize, subtask: usize, verdict: Verdict, test: FailedTest },
    /// No test of a subtask it is declared to fail breaks it.
    Passes { solution: usize, subtask: usize },
}

#[derive(Debug, Clone)]
pub struct FailedTest {
    pub generator: usize,
    pub seed: u64,
    pub input: String,
    pub expected: String,
    /// The solution's output, if it finished.
    pub output: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TestInfo {
    pub subtask: usize,
    pub generator: usize,
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

impl SubtaskResult {
    fn to_json(&self) -> Value {
        let verdicts: serde_json::Map<_, _> = self.verdicts.iter().map(|(verdict, &count)| (verdict.code().to_owned(), json!(count))).collect();
        json!({
            "passed": self.passed(),
            "verdicts": verdicts,
            "skipped": self.skipped,
            "time_ms": self.time_ms,
            "memory_kb": self.memory_kb,
            "first_failure": self.first_failure,
        })
    }
}

impl Mismatch {
    fn to_json(&self) -> Value {
        match self {
            Self::Fails { solution, subtask, verdict, test } => json!({
                "kind": "fails",
                "solution": solution,
                "subtask": subtask,
                "verdict": verdict.code(),
                "test": {
                    "generator": test.generator,
                    "seed": seed(test.seed),
                    "input": test.input,
                    "expected": test.expected,
                    "output": test.output,
                },
            }),
            Self::Passes { solution, subtask } => json!({ "kind": "passes", "solution": solution, "subtask": subtask }),
        }
    }
}

impl Report {
    /// The run's error, if it had one.
    pub fn into_result(self) -> crate::Result<()> {
        self.error.map_or(Ok(()), Err)
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let results = |results: &[SubtaskResult]| results.iter().map(SubtaskResult::to_json).collect::<Vec<_>>();
        let report = json!({
            "passed": self.error.is_none(),
            "error": self.error.as_ref().map(|err| json!({ "kind": err.kind(), "message": err.to_string() })),
            "seed": self.seed.map(seed),
            "subtasks": self.subtasks.iter().map(|subtask| json!({ "name": subtask.name, "points": subtask.points })).collect::<Vec<_>>(),
            "official": results(&self.official),
            "partials": self
                .partials
                .iter()
                .map(|partial| json!({ "name": partial.name, "declared": partial.declared, "results": results(&partial.results) }))
                .collect::<Vec<_>>(),
            "mismatches": self.mismatches.iter().map(Mismatch::to_json).collect::<Vec<_>>(),
            "tests": self
                .tests
                .iter()
                .map(|test| {
                    json!({
                        "subtask": test.subtask,
                        "generator": test.generator,
                        "seed": seed(test.seed),
                        "input_file": test.input_file,
                        "output_file": test.output_file,
                    })
                })
                .collect::<Vec<_>>(),
        });
        report.to_string()
    }
}
