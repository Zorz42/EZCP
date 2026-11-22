use thiserror::Error;

/// Subtask, partial solution and generator numbers count from one, except where a message says otherwise.
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error: {err} with file: {file}")]
    IOError { err: std::io::Error, file: String },

    #[error("Zip Error: {err}")]
    ZipError { err: zip::result::ZipError },

    #[error("Test file {path} already exists")]
    TestAlreadyExists { path: String },

    #[error(
        "C++ compiler is not found. Make sure to install it first. If it is already installed, \
    specify the path to compiler with the GCC_PATH environment variable."
    )]
    CompilerNotFound,

    #[error("Compiler error: {stderr}\n{stdout}")]
    CompilerError { stderr: String, stdout: String },

    /// A fault of the harness, not of the solution.
    #[error("Could not measure a solution's run: {details}")]
    TimerFailed { details: String },

    #[error("Solution timed out on test {test_path} (generator {gen_id})")]
    SolutionTimedOut { test_path: String, gen_id: usize },

    #[error("Solution exceeded the memory limit on test {test_path} (generator {gen_id})")]
    SolutionOutOfMemory { test_path: String, gen_id: usize },

    /// Also a non-zero exit code or far too much output.
    #[error("Solution crashed on test {test_path} (generator {gen_id})")]
    SolutionCrash { test_path: String, gen_id: usize },

    /// The checker rejected the official solution's own output.
    #[error("Solution produces wrong answer on {test_path} (generator {gen_id})")]
    SolutionFailed { test_path: String, gen_id: usize },

    #[error("Partial solution {partial_number} ({partial_name}) passes subtask {subtask_number} ({subtask_name}), which it is not supposed to pass")]
    PartialSolutionPassesExtraSubtask {
        subtask_number: usize,
        partial_number: usize,
        partial_name: String,
        subtask_name: String,
    },

    #[error("Partial solution {partial_number} ({partial_name}) does not pass subtask {subtask_number} ({subtask_name}) ({verdict}) (generator {gen_id}).")]
    PartialSolutionFailsSubtask {
        subtask_number: usize,
        partial_number: usize,
        partial_name: String,
        subtask_name: String,
        /// `WA`, `TLE`, `MLE` or `RTE`.
        verdict: String,
        gen_id: usize,
    },

    #[error("Missing solution")]
    MissingSolution,

    #[error("Invalid arguments: {details}")]
    InvalidArguments { details: String },

    #[error("Invalid test stub: {details}")]
    InvalidStub { details: String },

    #[error("This test is not the one the stub was written for: {details}")]
    StubMismatch { details: String },

    #[error(
        "Generator {gen_id} of subtask {subtask_number} is not reproducible: running it again with seed {seed} produced a different test on attempt \
         {attempt} of {attempts} ({details}). A generator has to take all of its randomness from the Rng it is given - anything else (rand::rng(), \
         the clock, a value captured while the task was being described, iterating a HashMap) makes a test that cannot be rebuilt from its seed."
    )]
    GeneratorNotReproducible {
        subtask_number: usize,
        gen_id: usize,
        seed: String,
        attempt: usize,
        attempts: usize,
        details: String,
    },

    #[error("Generator {gen_id} of subtask {subtask_number} panicked on seed {seed}: {message}")]
    GeneratorPanicked {
        subtask_number: usize,
        gen_id: usize,
        seed: String,
        message: String,
    },

    #[error("The validator of subtask {subtask_number} rejected the test of generator {gen_id} on seed {seed}: {message}")]
    ValidatorRejected {
        subtask_number: usize,
        gen_id: usize,
        seed: String,
        message: String,
    },

    /// Only with keep-going, which records every mismatch instead of stopping at the first.
    #[error("{count} declarations of partial solutions do not match what they do; the report lists them")]
    DeclarationsMismatch { count: usize },

    /// `subtask_number` is the 0-based index the solution named.
    #[error("Partial solution {partial_number} ({partial_name}) is declared to pass subtask index {subtask_number}, but the task only has {num_subtasks} subtasks (subtask indices are 0-based).")]
    InvalidSubtaskIndex {
        partial_number: usize,
        partial_name: String,
        subtask_number: usize,
        num_subtasks: usize,
    },
}

impl Error {
    /// The variant's name, for machine-readable reports.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::IOError { .. } => "IOError",
            Self::ZipError { .. } => "ZipError",
            Self::TestAlreadyExists { .. } => "TestAlreadyExists",
            Self::CompilerNotFound => "CompilerNotFound",
            Self::CompilerError { .. } => "CompilerError",
            Self::TimerFailed { .. } => "TimerFailed",
            Self::SolutionTimedOut { .. } => "SolutionTimedOut",
            Self::SolutionOutOfMemory { .. } => "SolutionOutOfMemory",
            Self::SolutionCrash { .. } => "SolutionCrash",
            Self::SolutionFailed { .. } => "SolutionFailed",
            Self::PartialSolutionPassesExtraSubtask { .. } => "PartialSolutionPassesExtraSubtask",
            Self::PartialSolutionFailsSubtask { .. } => "PartialSolutionFailsSubtask",
            Self::MissingSolution => "MissingSolution",
            Self::InvalidArguments { .. } => "InvalidArguments",
            Self::InvalidStub { .. } => "InvalidStub",
            Self::StubMismatch { .. } => "StubMismatch",
            Self::GeneratorNotReproducible { .. } => "GeneratorNotReproducible",
            Self::GeneratorPanicked { .. } => "GeneratorPanicked",
            Self::ValidatorRejected { .. } => "ValidatorRejected",
            Self::DeclarationsMismatch { .. } => "DeclarationsMismatch",
            Self::InvalidSubtaskIndex { .. } => "InvalidSubtaskIndex",
        }
    }

    pub(crate) fn io(file: &std::path::Path) -> impl FnOnce(std::io::Error) -> Self + '_ {
        |err| Self::IOError {
            err,
            file: file.display().to_string(),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A panic's message, if it has one.
pub fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|message| (*message).to_owned()))
        .unwrap_or_else(|| "a panic without a message".to_owned())
}
