use thiserror::Error;

/// Everything that can go wrong while building a task's test data.
///
/// Most variants carry the test and the generator that produced it, because the
/// first thing you want to know about a failure is which generator to go and
/// look at.
#[derive(Error, Debug)]
pub enum Error {
    /// A file could not be read or written.
    #[error("IO Error: {err} with file: {file}")]
    IOError {
        /// The underlying filesystem error.
        err: std::io::Error,
        /// The file being read or written when it happened.
        file: String,
    },

    /// The tests archive could not be written.
    #[error("Zip Error: {err}")]
    ZipError {
        /// The underlying error from the zip writer.
        err: zip::result::ZipError,
    },

    /// Two tests wanted the same file name.
    ///
    /// Writing the second one would destroy the first, so the run stops instead.
    /// Reach for [`Task::with_get_input_file_name`](crate::Task::with_get_input_file_name)
    /// or [`Task::with_get_output_file_name`](crate::Task::with_get_output_file_name)
    /// if the naming scheme is what needs changing.
    #[error("Test file {path} already exists")]
    TestAlreadyExists {
        /// The path that was about to be written twice.
        path: String,
    },

    /// No C++ compiler could be found on `PATH` or in the usual install locations.
    #[error(
        "C++ compiler is not found. Make sure to install it first. If it is already installed, \
    specify the path to compiler with the GCC_PATH environment variable."
    )]
    CompilerNotFound,

    /// A solution did not compile.
    #[error("Compiler error: {stderr}\n{stdout}")]
    CompilerError {
        /// What the compiler wrote to stderr, which is where the diagnostics are.
        stderr: String,
        /// What the compiler wrote to stdout.
        stdout: String,
    },

    /// The timer that runs and measures a solution did not report a usable verdict.
    ///
    /// This is a fault in the harness rather than in the solution: the timer is
    /// expected to end with a marker on stderr, and either it was missing or it
    /// could not be parsed.
    #[error("Could not measure a solution's run: {details}")]
    TimerFailed {
        /// What was wrong with the timer's output.
        details: String,
    },

    /// The official solution used more CPU time than the time limit allows.
    #[error("Solution timed out on test {test_path} (generator {gen_id})")]
    SolutionTimedOut {
        /// The test it was running.
        test_path: String,
        /// The generator that produced that test.
        gen_id: usize,
    },

    /// The official solution exited abnormally — a signal, or a non-zero status.
    #[error("Solution crashed on test {test_path} (generator {gen_id})")]
    SolutionCrash {
        /// The test it was running.
        test_path: String,
        /// The generator that produced that test.
        gen_id: usize,
    },

    /// The official solution ran, but the checker rejected its answer.
    ///
    /// Only reachable when the task has a custom checker, since without one the
    /// official solution defines the correct answer by construction.
    #[error("Solution produces wrong answer on {test_path} (generator {gen_id})")]
    SolutionFailed {
        /// The test it was running.
        test_path: String,
        /// The generator that produced that test.
        gen_id: usize,
    },

    /// A partial solution passed a subtask it declared it would fail.
    ///
    /// Either the partial solution is better than you thought, or the subtask's
    /// constraints are too loose to separate it from a correct one. Both are
    /// worth knowing before the task is used.
    #[error("Partial solution {partial_number} ({partial_name}) passes subtask {subtask_number} ({subtask_name}), which it is not supposed to pass")]
    PartialSolutionPassesExtraSubtask {
        /// Index of the subtask it was not supposed to pass.
        subtask_number: usize,
        /// Index of the partial solution.
        partial_number: usize,
        /// Name of the partial solution.
        partial_name: String,
        /// Name of the subtask.
        subtask_name: String,
    },

    /// A partial solution failed a subtask it declared it would pass.
    #[error("Partial solution {partial_number} ({partial_name}) does not pass subtask {subtask_number} ({subtask_name}) ({verdict}) (generator {gen_id}).")]
    PartialSolutionFailsSubtask {
        /// Index of the subtask it was supposed to pass.
        subtask_number: usize,
        /// Index of the partial solution.
        partial_number: usize,
        /// Name of the partial solution.
        partial_name: String,
        /// Name of the subtask.
        subtask_name: String,
        /// How it failed: wrong answer, a timeout or a crash.
        verdict: String,
        /// The generator that produced the test it failed on.
        gen_id: usize,
    },

    /// The task has no official solution, so there is nothing to produce the
    /// expected outputs with.
    #[error("Missing solution")]
    MissingSolution,

    /// The command line could not be parsed. See [`CliOptions`](crate::CliOptions).
    #[error("Invalid arguments: {details}")]
    InvalidArguments {
        /// What was wrong with the arguments.
        details: String,
    },

    /// A [test stub](crate::Stub) could not be read, or did not name a test this
    /// task has.
    #[error("Invalid test stub: {details}")]
    InvalidStub {
        /// What was wrong with the stub.
        details: String,
    },

    /// A stub was read successfully, but rebuilding it produced something other
    /// than what it was written for.
    ///
    /// Serving it would hand out test data that was never verified, so it is
    /// refused rather than used.
    #[error("This test is not the one the stub was written for: {details}")]
    StubMismatch {
        /// How the rebuilt test and the stub disagree.
        details: String,
    },

    /// A generator produced a different test the second time it was run with the
    /// same seed.
    ///
    /// The seed is then worthless — nothing can rebuild the test from it — so the
    /// run fails rather than writing a stub that lies. See the crate-level
    /// documentation on where a generator's randomness has to come from.
    #[error(
        "Generator {gen_id} of subtask {subtask_number} is not reproducible: running it again with seed {seed} produced a different test on attempt \
         {attempt} of {attempts} ({details}). A generator has to take all of its randomness from the Rng it is given - anything else (rand::rng(), \
         the clock, a value captured while the task was being described, iterating a HashMap) makes a test that cannot be rebuilt from its seed."
    )]
    GeneratorNotReproducible {
        /// Index of the subtask the generator belongs to.
        subtask_number: usize,
        /// Index of the generator within that subtask.
        gen_id: usize,
        /// The seed that failed to reproduce its test, in hexadecimal.
        seed: String,
        /// Which attempt disagreed.
        attempt: usize,
        /// How many attempts were made in total.
        attempts: usize,
        /// Where the two tests first differ.
        details: String,
    },

    /// A partial solution named a subtask index the task does not have.
    ///
    /// Almost always an off-by-one: subtask indices are 0-based.
    #[error("Partial solution {partial_number} ({partial_name}) is declared to pass subtask index {subtask_number}, but the task only has {num_subtasks} subtasks (subtask indices are 0-based).")]
    InvalidSubtaskIndex {
        /// Index of the partial solution.
        partial_number: usize,
        /// Name of the partial solution.
        partial_name: String,
        /// The subtask index it named.
        subtask_number: usize,
        /// How many subtasks the task actually has.
        num_subtasks: usize,
    },
}

/// A `Result` whose error defaults to [`enum@Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
