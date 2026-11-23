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

/// Keeps the better: a pass over a failure, then the faster.
pub const fn better(first: Outcome, second: Outcome) -> Outcome {
    match (first.verdict, second.verdict) {
        (Verdict::Ok, Verdict::Ok) if second.time_ms < first.time_ms => second,
        (Verdict::Ok, _) => first,
        (_, Verdict::Ok) => second,
        _ => first,
    }
}

impl<T: ToOutput> Task<T> {
    /// Whether an outcome is close enough to the time limit to be measured again.
    fn near_the_limit(&self, outcome: Outcome) -> bool {
        outcome.verdict == Verdict::TimedOut || (outcome.verdict == Verdict::Ok && 2 * outcome.time_ms > self.time_limit_ms)
    }

    /// Runs and judges the jobs. `skip` is asked right before a job would start, `on_outcome` told
    /// each outcome (`None` if skipped) in the order of the jobs. A run near the time limit that was
    /// not cached is repeated `retries` times and the best of them kept, which the runner caches.
    pub(crate) fn judge_runs(&self, runner: &Runner, jobs: &[Judged], retries: usize, skip: &dyn Fn(usize) -> bool, mut on_outcome: impl FnMut(usize, Option<Outcome>)) -> Result<()> {
        let limits = self.limits();
        let mut handles = jobs
            .iter()
            .enumerate()
            .map(|(idx, job)| (!skip(idx)).then(|| runner.spawn_checked(job.program, job.input, job.expected, &self.checker, limits)).transpose())
            .collect::<cpprunner::Result<Vec<Option<RunHandle<RunResult<bool>>>>>>()?;
        let mut near_the_limit = Vec::new();
        for idx in 0..jobs.len() {
            let Some(handle) = handles[idx].take() else {
                on_outcome(idx, None);
                continue;
            };
            if skip(idx) {
                handle.cancel();
            }
            let cached = handle.is_cached();
            let outcome = match handle.wait() {
                Ok(verdict) => Some(Outcome::from(verdict)),
                Err(cpprunner::Error::Cancelled) => None,
                Err(err) => return Err(err.into()),
            };
            match outcome {
                // Told once it has been measured again.
                Some(outcome) if retries > 0 && !cached && self.near_the_limit(outcome) => near_the_limit.push((idx, outcome)),
                _ => on_outcome(idx, outcome),
            }
            for (later, handle) in handles.iter().enumerate().skip(idx + 1) {
                if let Some(handle) = handle
                    && skip(later)
                {
                    handle.cancel();
                }
            }
        }

        let mut best: Vec<_> = near_the_limit.iter().map(|&(_, outcome)| outcome).collect();
        for _ in 0..if near_the_limit.is_empty() { 0 } else { retries } {
            let handles = near_the_limit
                .iter()
                .map(|&(idx, _)| runner.spawn_checked_fresh(jobs[idx].program, jobs[idx].input, jobs[idx].expected, &self.checker, limits))
                .collect::<cpprunner::Result<Vec<_>>>()?;
            for (best, handle) in best.iter_mut().zip(handles) {
                *best = better(*best, Outcome::from(handle.wait()?));
            }
        }
        for (&(idx, _), outcome) in near_the_limit.iter().zip(best) {
            on_outcome(idx, Some(outcome));
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
