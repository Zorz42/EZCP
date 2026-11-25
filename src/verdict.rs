use crate::{Error, Result};
use console::style;
use cpprunner::RunResult;
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

impl From<&RunResult<bool>> for Verdict {
    fn from(result: &RunResult<bool>) -> Self {
        match result {
            RunResult::Ok { output: true, .. } => Self::Ok,
            RunResult::Ok { output: false, .. } => Self::WrongAnswer,
            RunResult::TimedOut => Self::TimedOut,
            RunResult::MemoryLimitExceeded => Self::MemoryLimitExceeded,
            RunResult::ExitCode(_) | RunResult::Signal(_) | RunResult::OutputLimitExceeded => Self::Crashed,
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
