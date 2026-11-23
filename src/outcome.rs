//! Judged runs. With the outcome cache on, the runner keeps their verdicts in the build folder.

use crate::{Error, Result, Task, ToOutput};
use console::style;
use cpprunner::{RunHandle, RunResult, Runner};
use std::fmt::Display;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    Ok,
    TimedOut,
    MemoryLimitExceeded,
    Crashed,
    WrongAnswer,
}

impl Verdict {
    /// `OK`, `TLE`, `MLE`, `RTE` or `WA`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::TimedOut => "TLE",
            Self::MemoryLimitExceeded => "MLE",
            Self::Crashed => "RTE",
            Self::WrongAnswer => "WA",
        }
    }
}

impl serde::Serialize for Verdict {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

impl Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = style(self.code()).bright().bold();
        write!(f, "{}", if *self == Self::Ok { code.green() } else { code.red() })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub verdict: Verdict,
    /// Zero unless `Ok`.
    pub time_ms: i32,
    pub memory_kb: u64,
}

/// A program's run on an input, judged against `expected`.
#[derive(Clone, Copy)]
pub struct Judged<'test> {
    pub program: &'test str,
    pub input: &'test str,
    pub expected: &'test str,
}

impl From<RunResult<bool>> for Outcome {
    fn from(result: RunResult<bool>) -> Self {
        let failed = |verdict| Self { verdict, time_ms: 0, memory_kb: 0 };
        match result {
            RunResult::Ok { time_ms, memory_kb, output: true } => Self {
                verdict: Verdict::Ok,
                time_ms: i32::try_from(time_ms).unwrap_or(i32::MAX),
                memory_kb,
            },
            RunResult::Ok { output: false, .. } => failed(Verdict::WrongAnswer),
            RunResult::TimedOut => failed(Verdict::TimedOut),
            RunResult::MemoryLimitExceeded => failed(Verdict::MemoryLimitExceeded),
            RunResult::ExitCode(_) | RunResult::Signal(_) | RunResult::OutputLimitExceeded => failed(Verdict::Crashed),
        }
    }
}

impl<T: ToOutput> Task<T> {
    /// Runs and judges the jobs. `skip` is asked right before a job would start, `on_outcome` told
    /// each outcome (`None` if skipped) in the order of the jobs.
    pub(crate) fn judge_runs(&self, runner: &Runner, jobs: &[Judged], skip: &dyn Fn(usize) -> bool, mut on_outcome: impl FnMut(usize, Option<Outcome>)) -> Result<()> {
        let limits = self.limits();
        let mut handles = jobs
            .iter()
            .enumerate()
            .map(|(idx, job)| (!skip(idx)).then(|| runner.spawn_checked(job.program, job.input, job.expected, &self.checker, limits)).transpose())
            .collect::<cpprunner::Result<Vec<Option<RunHandle<RunResult<bool>>>>>>()?;
        for idx in 0..jobs.len() {
            let Some(handle) = handles[idx].take() else {
                on_outcome(idx, None);
                continue;
            };
            if skip(idx) {
                handle.cancel();
            }
            match handle.wait() {
                Ok(result) => on_outcome(idx, Some(Outcome::from(result))),
                Err(cpprunner::Error::Cancelled) => on_outcome(idx, None),
                Err(err) => return Err(err.into()),
            }
            for (later, handle) in handles.iter().enumerate().skip(idx + 1) {
                if let Some(handle) = handle
                    && skip(later)
                {
                    handle.cancel();
                }
            }
        }
        Ok(())
    }
}

/// The output, or the error for the official solution failing.
pub fn official_output(result: RunResult, test_path: &str, gen_id: usize) -> Result<String> {
    let test_path = test_path.to_owned();
    match result {
        RunResult::Ok { output, .. } => Ok(output),
        RunResult::TimedOut => Err(Error::SolutionTimedOut { test_path, gen_id }),
        RunResult::MemoryLimitExceeded => Err(Error::SolutionOutOfMemory { test_path, gen_id }),
        RunResult::ExitCode(_) | RunResult::Signal(_) | RunResult::OutputLimitExceeded => Err(Error::SolutionCrash { test_path, gen_id }),
    }
}
