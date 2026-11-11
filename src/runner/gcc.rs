use crate::Error::CompilerNotFound;
use crate::{Error, Result};
use log::debug;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Without the `\\?\` prefix on Windows. Use it for every compared path, or equal paths differ.
pub fn canonicalize(path: &Path) -> Result<PathBuf> {
    dunce::canonicalize(path).map_err(Error::io(path))
}

fn find_gcc() -> Result<PathBuf> {
    if let Ok(gcc_path) = std::env::var("GCC_PATH")
        && !gcc_path.is_empty()
    {
        // A full path or a program name.
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

/// Not every distribution installs the static libraries with the compiler.
fn has_static_libraries(compiler: &Path) -> bool {
    ["libstdc++.a", "libc.a"].iter().all(|library| {
        // The full path of a library the compiler finds, else just its name.
        let found = Command::new(compiler).arg(format!("-print-file-name={library}")).output();
        found.is_ok_and(|output| Path::new(String::from_utf8_lossy(&output.stdout).trim()).is_absolute())
    })
}

pub struct Gcc {
    /// Also keys cached binaries.
    pub path: PathBuf,
    /// Found out by the first compile, so a run that compiles nothing does not ask.
    links_statically: OnceLock<bool>,
}

impl Gcc {
    pub fn new() -> Result<Self> {
        Ok(Self {
            path: find_gcc()?,
            links_statically: OnceLock::new(),
        })
    }

    /// Absolute, next to `source`.
    pub fn executable_path(source: &Path) -> Result<PathBuf> {
        let mut executable = source.with_extension(if cfg!(windows) { "exe" } else { "" });
        // Else a source without an extension would be overwritten.
        if executable == source {
            let mut file_name = executable.file_name().unwrap_or_default().to_os_string();
            file_name.push("_bin");
            executable.set_file_name(file_name);
        }

        let file_name = executable.file_name().ok_or_else(|| Error::IOError {
            err: std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name"),
            file: executable.display().to_string(),
        })?;
        // The binary usually does not exist yet, so only its directory can be canonicalized.
        let parent = executable.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
        Ok(canonicalize(parent)?.join(file_name))
    }

    pub fn compile(&self, source: &Path, executable: &Path) -> Result<()> {
        let mut command = Command::new(&self.path);
        command.args(["-std=c++17", "-O2"]);

        #[cfg(windows)]
        {
            // No dependency on the MinGW DLLs being on PATH.
            command.arg("-static");
            // MinGW's default stack is 2 MB. The timer raises the limit on Linux instead.
            command.arg("-Wl,--stack,536870912");
        }

        #[cfg(target_os = "macos")]
        {
            // 512 MB instead of 8: macOS caps what setrlimit can raise it to.
            command.arg("-Wl,-stack_size,0x20000000");
        }

        // Not `#[cfg]`, which would leave `links_statically` unused elsewhere.
        if cfg!(target_os = "linux") {
            // A static program starts about a millisecond sooner, half of a short run. Without the
            // static libraries, unused ones are still left out: for the timer, all of the C++ runtime.
            let links_statically = *self.links_statically.get_or_init(|| has_static_libraries(&self.path));
            command.arg(if links_statically { "-static" } else { "-Wl,--as-needed" });
        }

        command.arg(source).arg("-o").arg(executable);

        #[cfg(windows)]
        {
            // For the timer's CommandLineToArgvW; it must come after the source.
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
