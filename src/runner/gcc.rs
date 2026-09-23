use crate::Error::CompilerNotFound;
use crate::{Error, Result};
use log::debug;
use std::path::{Path, PathBuf};

/// Like [`std::fs::canonicalize`], but without the `\\?\` prefix on Windows. Use
/// it for every path that is compared, or equal paths can compare unequal.
pub fn canonicalize(path: &Path) -> Result<PathBuf> {
    dunce::canonicalize(path).map_err(|err| Error::IOError {
        err,
        file: path.to_string_lossy().into_owned(),
    })
}

fn find_gcc() -> Result<PathBuf> {
    if let Ok(gcc_path) = std::env::var("GCC_PATH")
        && !gcc_path.is_empty()
    {
        // Accepts a full path or a program name.
        return which::which(&gcc_path).map_or_else(|_| Err(CompilerNotFound), Ok);
    }

    let candidates = if cfg!(windows) {
        ["g++", "mingw32-g++", "x86_64-w64-mingw32-g++", "c++"].as_slice()
    } else {
        ["g++", "c++", "clang++"].as_slice()
    };

    for candidate in candidates {
        if let Ok(gcc_path) = which::which(candidate) {
            return Ok(gcc_path);
        }
    }

    #[cfg(windows)]
    {
        // Installers do not always add these to PATH.
        let possible_dirs = [
            "C:\\msys64\\ucrt64\\bin",
            "C:\\msys64\\mingw64\\bin",
            "C:\\msys64\\mingw32\\bin",
            "C:\\msys32\\mingw32\\bin",
            "C:\\MinGW\\bin",
            "C:\\mingw64\\bin",
            "C:\\mingw-w64\\bin",
            "C:\\ProgramData\\chocolatey\\bin",
            "C:\\Program Files\\mingw64\\bin",
        ];

        for dir in possible_dirs {
            for candidate in candidates {
                let path = PathBuf::from(dir).join(format!("{candidate}.exe"));
                if path.is_file() {
                    return Ok(path);
                }
            }
        }
    }

    Err(CompilerNotFound)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum GccStandard {
    Cpp98,
    Cpp11,
    Cpp14,
    Cpp17,
    Cpp20,
    Cpp23,
}

impl GccStandard {
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Cpp98 => "c++98",
            Self::Cpp11 => "c++11",
            Self::Cpp14 => "c++14",
            Self::Cpp17 => "c++17",
            Self::Cpp20 => "c++20",
            Self::Cpp23 => "c++23",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum GccOptimization {
    Level1,
    Level2,
    Level3,
    Small,
    Fast,
}

impl GccOptimization {
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Level1 => "1",
            Self::Level2 => "2",
            Self::Level3 => "3",
            Self::Small => "s",
            Self::Fast => "fast",
        }
    }
}

/// Hashes the compiler and its configured flags, to key cached binaries.
#[derive(Hash)]
pub struct Gcc {
    path: PathBuf,
    pub standard: Option<GccStandard>,
    pub optimization: Option<GccOptimization>,
}

impl Gcc {
    pub fn new() -> Result<Self> {
        Ok(Self {
            path: find_gcc()?,
            standard: None,
            optimization: None,
        })
    }

    /// The absolute path of the binary, `.exe` on Windows. Creates its directory.
    pub fn transform_output_file(source_file: &PathBuf, output_file: Option<&PathBuf>) -> Result<PathBuf> {
        let mut output_file = output_file.map_or(source_file, |path| path).clone();
        if cfg!(windows) {
            output_file.set_extension("exe");
        } else {
            output_file.set_extension("");
        }

        // Otherwise a source without an extension would be overwritten by its binary.
        if output_file == *source_file {
            let mut file_name = output_file.file_name().unwrap_or_default().to_os_string();
            file_name.push("_bin");
            output_file.set_file_name(file_name);
        }

        let parent = match output_file.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        };
        if !parent.exists() {
            std::fs::create_dir_all(&parent).map_err(|err| Error::IOError {
                err,
                file: parent.to_string_lossy().into_owned(),
            })?;
        }

        // The binary itself usually does not exist yet, so only its directory can
        // be canonicalized.
        let file_name = output_file.file_name().ok_or_else(|| Error::IOError {
            err: std::io::Error::new(std::io::ErrorKind::InvalidInput, "output path has no file name"),
            file: output_file.to_string_lossy().into_owned(),
        })?;
        Ok(canonicalize(&parent)?.join(file_name))
    }

    /// Returns the absolute path of the binary.
    pub fn compile(&self, source_file: &Path, output_file: Option<&PathBuf>) -> Result<PathBuf> {
        let source_file = canonicalize(source_file)?;
        let output_file = Self::transform_output_file(&source_file, output_file)?;

        let mut command = std::process::Command::new(&self.path);

        if let Some(standard) = self.standard {
            command.arg(format!("-std={}", standard.as_str()));
        }

        if let Some(optimization) = self.optimization {
            command.arg(format!("-O{}", optimization.as_str()));
        }

        #[cfg(windows)]
        {
            // No dependency on the MinGW DLLs being on PATH.
            command.arg("-static");
            // Deep recursion needs more than MinGW's default 2 MB stack. The
            // timer raises the stack limit on Linux instead.
            command.arg("-Wl,--stack,536870912");
        }

        #[cfg(target_os = "macos")]
        {
            // 512 MB instead of 8 MB; macOS caps what setrlimit can raise it to.
            command.arg("-Wl,-stack_size,0x20000000");
        }

        command.arg(&source_file).arg("-o").arg(&output_file);

        #[cfg(windows)]
        {
            // For the timer's CommandLineToArgvW. It must come after the source.
            command.arg("-lshell32");
        }

        debug!("Running command: {command:?}");
        let process = command.output().map_err(|err| Error::IOError {
            err,
            file: self.path.to_string_lossy().into_owned(),
        })?;

        if !process.status.success() {
            return Err(Error::CompilerError {
                stderr: String::from_utf8_lossy(&process.stderr).to_string(),
                stdout: String::from_utf8_lossy(&process.stdout).to_string(),
            });
        }

        if !output_file.exists() {
            return Err(Error::CompilerError {
                stderr: "Output file was not created".to_owned(),
                stdout: String::new(),
            });
        }

        Ok(output_file)
    }
}
