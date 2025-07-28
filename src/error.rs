use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{message}")]
    CustomError { message: String },

    #[error("IO Error: {err} with file: {file}")]
    IOError { err: std::io::Error, file: String },

    #[error("Snap Error: {err}")]
    SnapError { err: snap::Error },

    #[error("Bincode Error: {err}")]
    BincodeError { err: bincode::error::EncodeError },

    #[error("Zip Error: {err}")]
    ZipError { err: zip::result::ZipError },

    #[error("Test file {path} already exists")]
    TestAlreadyExists { path: String },

    #[error("Expected integer in input")]
    InputExpectedInteger,

    #[error("Expected float in input")]
    InputExpectedFloat,

    #[error("Expected string in input")]
    InputExpectedString,

    #[error("Expected end of input")]
    InputExpectedEnd,

    #[error("Expected {n} integers")]
    ExpectedIntegers { n: i32 },

    #[error("C++ compiler is not found. Make sure to install it first. If it is already installed, \
    specify the path to compiler with the GCC_PATH environment variable.")]
    CompilerNotFound,

    #[error("Compiler error: {stderr}\n{stdout}")]
    CompilerError { stderr: String, stdout: String },

    #[error("Solution timed out on test {test_path}")]
    SolutionTimedOut { test_path: String },

    #[error("Solution crashed on test {test_path}")]
    SolutionFailed { test_path: String },

    #[error("Partial solution {partial_number} passes extra subtask {subtask_number}")]
    PartialSolutionPassesExtraSubtask { subtask_number: usize, partial_number: usize },

    #[error("Partial solution {partial_number} does not pass subtask {subtask_number}.")]
    PartialSolutionFailsSubtask { subtask_number: usize, partial_number: usize },

    #[error("Missing solution")]
    MissingSolution { },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// macro bail that returns CustomError
#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::Error::CustomError { message: format!($($arg)*) });
    };
}
