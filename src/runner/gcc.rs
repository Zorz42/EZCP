use crate::Error::CompilerNotFound;
use crate::{Error, Result};
use log::debug;
use std::path::{Path, PathBuf};

/// Like [`std::fs::canonicalize`], but without the `\\?\` prefix on Windows. Use
/// it for every path that is compared, or equal paths can compare unequal.
pub fn canonicalize(path: &Path) -> Result<PathBuf> {
    dunce::canonicalize(path).map_err(Error::io(path))
}

fn find_gcc() -> Result<PathBuf> {
    if let Ok(gcc_path) = std::env::var("GCC_PATH")
        && !gcc_path.is_empty()
    {
        // Accepts a full path or a program name.
        return which::which(&gcc_path).map_err(|_not_found| CompilerNotFound);
    }

    let candidates = if cfg!(windows) {
        ["g++", "mingw32-g++", "x86_64-w64-mingw32-g++", "c++"].as_slice()
    } else {
        ["g++", "c++", "clang++"].as_slice()
    };

    if let Some(path) = candidates.iter().find_map(|candidate| which::which(candidate).ok()) {
        return Ok(path);
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

/// Hashes to the compiler's path, to key cached binaries.
#[derive(Hash)]
pub struct Gcc {
    path: PathBuf,
}

impl Gcc {
    pub fn new() -> Result<Self> {
        Ok(Self { path: find_gcc()? })
    }

    /// The absolute path of the binary for `source`: next to it, `.exe` on Windows.
    pub fn executable_path(source: &Path) -> Result<PathBuf> {
        let mut executable = source.with_extension(if cfg!(windows) { "exe" } else { "" });
        // A source without an extension would otherwise be overwritten by its binary.
        if executable == source {
            let mut file_name = executable.file_name().unwrap_or_default().to_os_string();
            file_name.push("_bin");
            executable.set_file_name(file_name);
        }

        let file_name = executable.file_name().ok_or_else(|| Error::IOError {
            err: std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name"),
            file: executable.display().to_string(),
        })?;
        // The binary itself usually does not exist yet, so only its directory can
        // be canonicalized.
        let parent = executable.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
        Ok(canonicalize(parent)?.join(file_name))
    }

    pub fn compile(&self, source: &Path, executable: &Path) -> Result<()> {
        let mut command = std::process::Command::new(&self.path);
        command.args(["-std=c++17", "-O2"]);

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

        command.arg(source).arg("-o").arg(executable);

        #[cfg(windows)]
        {
            // For the timer's CommandLineToArgvW. It must come after the source.
            command.arg("-lshell32");
        }

        debug!("Running command: {command:?}");
        let process = command.output().map_err(Error::io(&self.path))?;

        if !process.status.success() {
            return Err(Error::CompilerError {
                stderr: String::from_utf8_lossy(&process.stderr).into_owned(),
                stdout: String::from_utf8_lossy(&process.stdout).into_owned(),
            });
        }

        if !executable.exists() {
            return Err(Error::CompilerError {
                stderr: "Output file was not created".to_owned(),
                stdout: String::new(),
            });
        }

        Ok(())
    }
}
