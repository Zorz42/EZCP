use thiserror::Error;

/// An error while building or serving a task's tests.
#[derive(Error, Debug)]
pub enum Error {
    /// A file could not be read or written.
    #[error("IO Error: {err} with file: {file}")]
    IOError {
        /// The underlying error.
        err: std::io::Error,
        /// The file involved.
        file: String,
    },

    /// The tests archive could not be written.
    #[error("Zip Error: {err}")]
    ZipError {
        /// The underlying error.
        err: zip::result::ZipError,
    },

    /// Two tests were given the same file name.
    #[error("Test file {path} already exists")]
    TestAlreadyExists {
        /// The duplicated name.
        path: String,
    },

    /// No C++ compiler was found on `PATH`, in `GCC_PATH` or in the usual
    /// install locations.
    #[error(
        "C++ compiler is not found. Make sure to install it first. If it is already installed, \
    specify the path to compiler with the GCC_PATH environment variable."
    )]
    CompilerNotFound,

    /// A solution did not compile.
    #[error("Compiler error: {stderr}\n{stdout}")]
    CompilerError {
        /// The compiler's stderr.
        stderr: String,
        /// The compiler's stdout.
        stdout: String,
    },

    /// The timer could not run the solution or did not report a verdict. A
    /// fault of the harness, not of the solution.
    #[error("Could not measure a solution's run: {details}")]
    TimerFailed {
        /// What went wrong.
        details: String,
    },

    /// The official solution exceeded the time limit.
    #[error("Solution timed out on test {test_path} (generator {gen_id})")]
    SolutionTimedOut {
        /// The test.
        test_path: String,
        /// 1-based index of the generator that produced the test.
        gen_id: usize,
    },

    /// The official solution crashed, exited with a non-zero status or wrote
    /// far too much output.
    #[error("Solution crashed on test {test_path} (generator {gen_id})")]
    SolutionCrash {
        /// The test.
        test_path: String,
        /// 1-based index of the generator that produced the test.
        gen_id: usize,
    },

    /// The custom checker rejected the official solution's own output.
    #[error("Solution produces wrong answer on {test_path} (generator {gen_id})")]
    SolutionFailed {
        /// The test.
        test_path: String,
        /// 1-based index of the generator that produced the test.
        gen_id: usize,
    },

    /// A partial solution passed a subtask it is declared to fail.
    #[error("Partial solution {partial_number} ({partial_name}) passes subtask {subtask_number} ({subtask_name}), which it is not supposed to pass")]
    PartialSolutionPassesExtraSubtask {
        /// 1-based subtask index.
        subtask_number: usize,
        /// 1-based partial solution index.
        partial_number: usize,
        /// Name of the partial solution.
        partial_name: String,
        /// Name of the subtask.
        subtask_name: String,
    },

    /// A partial solution failed a subtask it is declared to pass.
    #[error("Partial solution {partial_number} ({partial_name}) does not pass subtask {subtask_number} ({subtask_name}) ({verdict}) (generator {gen_id}).")]
    PartialSolutionFailsSubtask {
        /// 1-based subtask index.
        subtask_number: usize,
        /// 1-based partial solution index.
        partial_number: usize,
        /// Name of the partial solution.
        partial_name: String,
        /// Name of the subtask.
        subtask_name: String,
        /// `WA`, `TLE` or `RTE`.
        verdict: String,
        /// 1-based index of the generator that produced the test.
        gen_id: usize,
    },

    /// The task has no official solution.
    #[error("Missing solution")]
    MissingSolution,

    /// The command line could not be parsed.
    #[error("Invalid arguments: {details}")]
    InvalidArguments {
        /// What was wrong.
        details: String,
    },

    /// A [stub](crate::Stub) could not be parsed or names a test the task does
    /// not have.
    #[error("Invalid test stub: {details}")]
    InvalidStub {
        /// What was wrong.
        details: String,
    },

    /// A rebuilt test does not match the hash in its stub.
    #[error("This test is not the one the stub was written for: {details}")]
    StubMismatch {
        /// How they differ.
        details: String,
    },

    /// A generator produced a different test when run again with the same seed.
    #[error(
        "Generator {gen_id} of subtask {subtask_number} is not reproducible: running it again with seed {seed} produced a different test on attempt \
         {attempt} of {attempts} ({details}). A generator has to take all of its randomness from the Rng it is given - anything else (rand::rng(), \
         the clock, a value captured while the task was being described, iterating a HashMap) makes a test that cannot be rebuilt from its seed."
    )]
    GeneratorNotReproducible {
        /// 1-based subtask index.
        subtask_number: usize,
        /// 1-based index of the generator within the subtask.
        gen_id: usize,
        /// The seed, in hexadecimal.
        seed: String,
        /// Which rebuild differed.
        attempt: usize,
        /// How many rebuilds were to be made.
        attempts: usize,
        /// Where the two tests first differ.
        details: String,
    },

    /// A partial solution is declared to pass a subtask that does not exist.
    #[error("Partial solution {partial_number} ({partial_name}) is declared to pass subtask index {subtask_number}, but the task only has {num_subtasks} subtasks (subtask indices are 0-based).")]
    InvalidSubtaskIndex {
        /// 1-based partial solution index.
        partial_number: usize,
        /// Name of the partial solution.
        partial_name: String,
        /// The 0-based subtask index it named.
        subtask_number: usize,
        /// How many subtasks the task has.
        num_subtasks: usize,
    },
}

/// A `Result` whose error defaults to [`enum@Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
