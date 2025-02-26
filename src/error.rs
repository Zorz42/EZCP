use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{message}")]
    CustomError { message: String },

    #[error("IO Error: {err}")]
    IOError { err: std::io::Error },

    #[error("Snap Error: {err}")]
    SnapError { err: snap::Error },

    #[error("Bincode Error: {err}")]
    BincodeError { err: bincode::Error },

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

    #[error("C++ compiler is not found. Specify the path to g++ with the GCC_PATH environment variable.")]
    CompilerNotFoundWindows,

    #[error("C++ compiler is not found. Please install g++ and make sure, it is in the path.")]
    CompilerNotFoundUnix,

    #[error("Compiler error: {stderr}\n{stdout}")]
    CompilerError { stderr: String, stdout: String },

    #[error("Solution timed out on test {test_path}")]
    SolutionTimedOut { test_path: String },

    #[error("Solution crashed on test {test_path}")]
    SolutionFailed { test_path: String },

    #[error("Solution returned wrong answer on test {test_path}")]
    SolutionWrongAnswer { test_path: String },

    #[error("Partial solution {partial_number} passes extra subtask {subtask_number}")]
    PartialSolutionPassesExtraSubtask { subtask_number: usize, partial_number: usize },

    #[error("Partial solution {partial_number} does not pass subtask {subtask_number}. Error: \"{message}\"")]
    PartialSolutionFailsSubtask { subtask_number: usize, partial_number: usize, message: String },

    #[error("Missing solution file: {path}")]
    MissingSolutionFile { path: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// macro bail that returns CustomError
#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::Error::CustomError { message: format!($($arg)*) });
    };
}
