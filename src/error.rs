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

    #[error("Solution timed out on test {test_path} (generator {gen_id})")]
    SolutionTimedOut { test_path: String, gen_id: usize },

    #[error("Solution crashed on test {test_path} (generator {gen_id})")]
    SolutionCrash { test_path: String, gen_id: usize },

    #[error("Solution produces wrong answer on {test_path} (generator {gen_id})")]
    SolutionFailed { test_path: String, gen_id: usize },

    #[error("Partial solution {partial_number} passes extra subtask {subtask_number} (generator {gen_id})")]
    PartialSolutionPassesExtraSubtask { subtask_number: usize, partial_number: usize, gen_id: usize },

    #[error("Partial solution {partial_number} does not pass subtask {subtask_number} ({verdict}) (generator {gen_id}).")]
    PartialSolutionFailsSubtask {
        subtask_number: usize,
        partial_number: usize,
        verdict: String,
        gen_id: usize,
    },

    #[error("Missing solution")]
    MissingSolution {},
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// macro bail that returns CustomError
#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::Error::CustomError { message: format!($($arg)*) });
    };
}
