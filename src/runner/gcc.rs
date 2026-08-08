use crate::Error::CompilerNotFound;
use crate::{Error, Result};
use log::debug;
use std::path::{Path, PathBuf};

/// Resolves a path to an absolute one without the `\\?\` prefix that
/// [`std::fs::canonicalize`] adds on Windows.
///
/// Everything that stores or compares paths has to go through this, otherwise a
/// verbatim and a plain form of the same path compare unequal and, for example,
/// the build folder cleanup would not recognise its own binaries.
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
        // Accept both a full path and a bare program name, and fail with the
        // "compiler not found" hint instead of an obscure spawn error later on.
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
        // Common toolchain locations that installers do not always add to PATH.
        let possible_dirs = [
            // MSYS2
            "C:\\msys64\\ucrt64\\bin",
            "C:\\msys64\\mingw64\\bin",
            "C:\\msys64\\mingw32\\bin",
            "C:\\msys32\\mingw32\\bin",
            // MinGW standalone
            "C:\\MinGW\\bin",
            "C:\\mingw64\\bin",
            "C:\\mingw-w64\\bin",
            // Chocolatey / winlibs
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

/// C++ standards supported by GCC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Optimization levels for the C++ compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Wrapper around the `g++` compiler.
pub struct Gcc {
    /// Absolute path to the `g++` executable.
    path: PathBuf,
    /// Language standard to use (e.g., -std=c++17).
    pub standard: Option<GccStandard>,
    /// Optimization level to use (e.g., -O2).
    pub optimization: Option<GccOptimization>,
}

impl Gcc {
    /// Locates the `g++` compiler on the system.
    pub fn new() -> Result<Self> {
        Ok(Self {
            path: find_gcc()?,
            standard: None,
            optimization: None,
        })
    }

    /// Predicts the output binary path for a given source file.
    ///
    /// This method ensures parent directories exist and handles platform-specific
    /// extensions (.exe on Windows). The returned path is absolute.
    pub fn transform_output_file(source_file: &PathBuf, output_file: Option<&PathBuf>) -> Result<PathBuf> {
        let mut output_file = output_file.map_or(source_file, |path| path).clone();
        if cfg!(windows) {
            output_file.set_extension("exe");
        } else {
            output_file.set_extension("");
        }

        // A source file without an extension would otherwise be overwritten by
        // its own binary.
        if output_file == *source_file {
            let mut file_name = output_file.file_name().unwrap_or_default().to_os_string();
            file_name.push("_bin");
            output_file.set_file_name(file_name);
        }

        // create the parent directory if it does not exist
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

        // Canonicalize the directory rather than the binary itself: the binary
        // usually does not exist yet, and creating a placeholder just to resolve
        // it would race with a compile running in parallel.
        let file_name = output_file.file_name().ok_or_else(|| Error::IOError {
            err: std::io::Error::new(std::io::ErrorKind::InvalidInput, "output path has no file name"),
            file: output_file.to_string_lossy().into_owned(),
        })?;
        Ok(canonicalize(&parent)?.join(file_name))
    }

    /// Compiles a C++ source file into an executable.
    ///
    /// Returns the absolute path to the generated binary.
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
            command.arg("-static"); // Use static linking on Windows to avoid DLL issues
            // MinGW defaults to a 2MB stack, far too little for the deep
            // recursion competitive programming solutions rely on. Unix gets the
            // same headroom from setrlimit / -stack_size.
            command.arg("-Wl,--stack,536870912");
        }

        #[cfg(target_os = "macos")]
        {
            // On macOS, the default stack size is small (8MB).
            // We increase it to 512MB for competitive programming.
            command.arg("-Wl,-stack_size,0x20000000");
        }

        command.arg(&source_file).arg("-o").arg(&output_file);
        // Do not override current_dir; pass absolute paths instead

        #[cfg(windows)]
        {
            // The timer calls CommandLineToArgvW. Shell32 is part of the default
            // MinGW link line, but ask for it explicitly so an unusual toolchain
            // configuration cannot break the build. Libraries have to follow the
            // objects that reference them.
            command.arg("-lshell32");
        }

        debug!("Running command: {command:?}");
        let process = command.output().map_err(|err| Error::IOError { err, file: String::new() })?;

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
