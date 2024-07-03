use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Test file already exists: {}", path.display())]
    TestFileAlreadyExists { path: PathBuf },

    #[error("Test file failed to copy: {} -> {} Error: {}", src.display(), dst.display(), err)]
    TestFileCopy { src: PathBuf, dst: PathBuf, err: std::io::Error },

    #[error("Failed to write test file: {} Error: {}", path.display(), err)]
    TestFileWrite { path: PathBuf, err: std::io::Error },

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