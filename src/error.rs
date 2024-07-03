use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{message}")]
    CustomError { message: String },

    #[error("File System Error: {err}")]
    FileSystemError { err: std::io::Error },

    #[error("Snap Error: {err}")]
    SnapError { err: snap::Error },

    #[error("Bincode Error: {err}")]
    BincodeError { err: bincode::Error },

    #[error("Zip Error: {err}")]
    ZipError { err: zip::result::ZipError },

    #[error("Test file {path} already exists")]
    TestAlreadyExists { path: PathBuf },

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
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// macro bail that returns CustomError
#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::Error::CustomError { message: format!($($arg)*) });
    };
}