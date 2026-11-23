//! Judged runs, and the cache of them that a task can keep in its build folder.
//!
//! A cached outcome is keyed by the program, the input, the output the checker compares against and
//! the limits, so it holds as long as the checker does not change.

use crate::stub::stable_hash;
use crate::{Error, Result, Task, ToOutput};
use console::style;
use cpprunner::{RunHandle, RunResult, Runner};
use std::collections::HashMap;
use std::fmt::Display;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const CACHE_FILE: &str = "outcomes.cache";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    Ok,
    TimedOut,
    MemoryLimitExceeded,
    Crashed,
    WrongAnswer,
}

impl Verdict {
    const ALL: [Self; 5] = [Self::Ok, Self::TimedOut, Self::MemoryLimitExceeded, Self::Crashed, Self::WrongAnswer];

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

    fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|verdict| verdict.code() == code)
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

type Key = (u64, u64, u64, i32, u64);

pub struct OutcomeCache {
    entries: Mutex<HashMap<Key, Outcome>>,
    file: Mutex<File>,
}

impl OutcomeCache {
    pub fn path(build_folder: &Path) -> PathBuf {
        build_folder.join(CACHE_FILE)
    }

    /// Lines it cannot read are ignored, as a run cut short may leave half of one.
    pub fn open(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let entries = text.lines().filter_map(parse_line).collect();
        let file = OpenOptions::new().create(true).append(true).open(path).map_err(Error::io(path))?;
        Ok(Self {
            entries: Mutex::new(entries),
            file: Mutex::new(file),
        })
    }

    fn get(&self, key: &Key) -> Option<Outcome> {
        self.entries.lock().ok()?.get(key).copied()
    }

    fn insert(&self, key: Key, outcome: Outcome) -> Result<()> {
        let (program, input, expected, time_ms, memory_kb) = key;
        let line = format!(
            "{program:016x} {input:016x} {expected:016x} {time_ms} {memory_kb} {} {} {}\n",
            outcome.verdict.code(),
            outcome.time_ms,
            outcome.memory_kb
        );
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(key, outcome);
        }
        let mut file = self.file.lock().map_err(|_poisoned| Error::TimerFailed {
            details: "the outcome cache was poisoned".to_owned(),
        })?;
        file.write_all(line.as_bytes()).map_err(Error::io(Path::new(CACHE_FILE)))
    }
}

fn parse_line(line: &str) -> Option<(Key, Outcome)> {
    let fields: Vec<_> = line.split_whitespace().collect();
    let &[program, input, expected, time_limit, memory_limit, verdict, time_ms, memory_kb] = fields.as_slice() else {
        return None;
    };
    let hex = |field: &str| u64::from_str_radix(field, 16).ok();
    let key = (hex(program)?, hex(input)?, hex(expected)?, time_limit.parse().ok()?, memory_limit.parse().ok()?);
    let outcome = Outcome {
        verdict: Verdict::parse(verdict)?,
        time_ms: time_ms.parse().ok()?,
        memory_kb: memory_kb.parse().ok()?,
    };
    Some((key, outcome))
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
    pub(crate) fn outcome(&self, result: RunResult, input: &str, expected: &str) -> Outcome {
        let failed = |verdict| Outcome { verdict, time_ms: 0, memory_kb: 0 };
        match result {
            RunResult::Ok { time_ms, memory_kb, output } if (self.checker)(input, expected, &output) => Outcome {
                verdict: Verdict::Ok,
                time_ms: time_ms.min(i32::MAX as u64) as i32,
                memory_kb,
            },
            RunResult::Ok { .. } => failed(Verdict::WrongAnswer),
            RunResult::TimedOut => failed(Verdict::TimedOut),
            RunResult::MemoryLimitExceeded => failed(Verdict::MemoryLimitExceeded),
            RunResult::Crashed(_) => failed(Verdict::Crashed),
        }
    }

    /// Whether an outcome is close enough to the time limit to be measured again.
    fn near_the_limit(&self, outcome: Outcome) -> bool {
        outcome.verdict == Verdict::TimedOut || (outcome.verdict == Verdict::Ok && 2 * outcome.time_ms > self.time_limit_ms)
    }

    /// Runs and judges the jobs the cache does not know. `skip` is asked right before a job would
    /// start, `on_outcome` told each outcome (`None` if skipped) in the order of the jobs, cached ones
    /// first. A fresh run near the time limit is repeated `retries` times and the best of them kept.
    pub(crate) fn judge_runs(
        &self,
        runner: &Runner,
        cache: Option<&OutcomeCache>,
        jobs: &[Judged],
        retries: usize,
        skip: &dyn Fn(usize) -> bool,
        mut on_outcome: impl FnMut(usize, Option<Outcome>),
    ) -> Result<()> {
        let limits = self.limits();
        let key = |job: &Judged| {
            let program = runner.program_hash(job.program);
            (program, stable_hash(job.input), stable_hash(job.expected), self.time_limit_ms, limits.memory_kb.unwrap_or(0))
        };
        let mut misses = Vec::new();
        for (idx, job) in jobs.iter().enumerate() {
            match cache.and_then(|cache| cache.get(&key(job))) {
                Some(outcome) => on_outcome(idx, Some(outcome)),
                None => misses.push(idx),
            }
        }

        let mut handles = misses
            .iter()
            .map(|&idx| (!skip(idx)).then(|| runner.spawn(jobs[idx].program, jobs[idx].input, limits)).transpose())
            .collect::<cpprunner::Result<Vec<Option<RunHandle>>>>()?;
        let mut near_the_limit = vec![None; misses.len()];
        for run in 0..misses.len() {
            let (idx, job) = (misses[run], &jobs[misses[run]]);
            let result = match handles[run].take() {
                Some(handle) if skip(idx) => {
                    handle.cancel();
                    handle.wait()
                }
                Some(handle) => handle.wait(),
                None => Err(cpprunner::Error::Cancelled),
            };
            let outcome = match result {
                Ok(result) => Some(self.outcome(result, job.input, job.expected)),
                Err(cpprunner::Error::Cancelled) => None,
                Err(err) => return Err(err.into()),
            };
            match outcome {
                // Told once it has been measured again.
                Some(outcome) if retries > 0 && self.near_the_limit(outcome) => near_the_limit[run] = Some(outcome),
                _ => {
                    if let (Some(cache), Some(outcome)) = (cache, outcome) {
                        cache.insert(key(job), outcome)?;
                    }
                    on_outcome(idx, outcome);
                }
            }
            for (later, handle) in handles.iter().enumerate().skip(run + 1) {
                if let Some(handle) = handle
                    && skip(misses[later])
                {
                    handle.cancel();
                }
            }
        }

        let again: Vec<_> = near_the_limit.iter().enumerate().filter_map(|(run, outcome)| Some((misses[run], (*outcome)?))).collect();
        let mut best: Vec<_> = again.iter().map(|&(_, outcome)| outcome).collect();
        for _ in 0..if again.is_empty() { 0 } else { retries } {
            let handles = again
                .iter()
                .map(|&(idx, _)| runner.spawn_fresh(jobs[idx].program, jobs[idx].input, limits))
                .collect::<cpprunner::Result<Vec<_>>>()?;
            for ((best, &(idx, _)), handle) in best.iter_mut().zip(&again).zip(handles) {
                let job = &jobs[idx];
                *best = better(*best, self.outcome(handle.wait()?, job.input, job.expected));
            }
        }
        for (&(idx, _), outcome) in again.iter().zip(best) {
            if let Some(cache) = cache {
                cache.insert(key(&jobs[idx]), outcome)?;
            }
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
        RunResult::Crashed(_) => Err(Error::SolutionCrash { test_path, gen_id }),
    }
}
