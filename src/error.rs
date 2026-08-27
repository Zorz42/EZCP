use thiserror::Error;

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

    #[error("Could not measure a solution's run: {details}")]
    TimerFailed { details: String },

    #[error("Solution timed out on test {test_path} (generator {gen_id})")]
    SolutionTimedOut { test_path: String, gen_id: usize },

    #[error("Solution crashed on test {test_path} (generator {gen_id})")]
    SolutionCrash { test_path: String, gen_id: usize },

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
        verdict: String,
        gen_id: usize,
    },

    #[error("Missing solution")]
    MissingSolution,

    #[error("Invalid arguments: {details}")]
    InvalidArguments { details: String },

    #[error("Invalid seed manifest: {details}")]
    InvalidManifest { details: String },

    #[error("The seed manifest does not match this task: {details}")]
    ManifestMismatch { details: String },

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

    #[error("Partial solution {partial_number} ({partial_name}) is declared to pass subtask index {subtask_number}, but the task only has {num_subtasks} subtasks (subtask indices are 0-based).")]
    InvalidSubtaskIndex {
        partial_number: usize,
        partial_name: String,
        subtask_number: usize,
        num_subtasks: usize,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
