use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Test file already exists: {}", path.display())]
    FileAlreadyExists { path: PathBuf },

    #[error("Test file failed to copy: {} -> {} Error: {}", src.display(), dst.display(), err)]
    FileCopy { src: PathBuf, dst: PathBuf, err: std::io::Error },

    #[error("Failed to write test file: {} Error: {}", path.display(), err)]
    FileWrite { path: PathBuf, err: std::io::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;