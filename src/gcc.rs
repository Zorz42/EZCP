use std::fs::exists;
use std::path::{Path, PathBuf};
use crate::Error::{CompilerNotFound};
use crate::{Error, Result};

fn find_gcc() -> Result<PathBuf> {
    if let Ok(gcc_path) = std::env::var("GCC_PATH") {
        return Ok(PathBuf::from(gcc_path));
    }

    #[cfg(unix)]
    {
        // use which to find gcc in the PATH
        which::which("g++").map_or(Err(CompilerNotFound), Ok)
    }
    #[cfg(windows)]
    {
        let candidates = [
            "g++",
            "mingw32-g++",
            "x86_64-w64-mingw32-g++",
            "c++",
            "cl",
        ];

        for candidate in candidates {
            if let Ok(gcc_path) = which::which(candidate) {
                return Ok(gcc_path);
            }
        }

        let possible_dirs = [
            // MSYS2
            "C:\\msys64\\mingw64\\bin",
            "C:\\msys64\\mingw32\\bin",
            "C:\\msys32\\mingw32\\bin",

            // MinGW standalone
            "C:\\MinGW\\bin",
            "C:\\mingw-w64\\bin",

            // Visual Studio (uncommon for gcc, but you may want cl.exe)
            "C:\\Program Files (x86)\\Microsoft Visual Studio\\2019\\Community\\VC\\Tools\\MSVC",
            "C:\\Program Files\\Microsoft Visual Studio\\2022\\Community\\VC\\Tools\\MSVC",
        ];

        for dir in possible_dirs {
            for candidate in &candidates {
                let path = PathBuf::from(dir).join(format!("{candidate}.exe"));
                if path.exists() {
                    return Ok(path);
                }
            }
        }

        Err(CompilerNotFound)
    }
}

pub struct Gcc {
    path: PathBuf,
    flags: Vec<String>,
}

impl Gcc {
    pub fn new() -> Result<Self> {
        Ok(Self {
            path: find_gcc()?,
            flags: Vec::new(),
        })
    }

    pub fn add_flag<S: Into<String>>(&mut self, flag: S) {
        self.flags.push(flag.into());
    }

    /// Calls `gcc` to compile the source file.
    /// If output_file is None, it will use the source file name with an appropriate extension.
    pub fn compile(&self, source_file: &Path, output_file: Option<&Path>) -> Result<PathBuf> {
        let mut output_file = output_file.map_or(source_file, |p| p).to_owned();
        #[cfg(windows)]
        {
            output_file.set_extension("exe");
        }
        #[cfg(unix)]
        {
            output_file.set_extension("");
        }

        let mut command = std::process::Command::new(&self.path);
        for flag in &self.flags {
            command.arg(flag);
        }
        command.arg(source_file).arg("-o").arg(&output_file);
        if let Some(parent) = self.path.parent() {
            command.current_dir(parent);
        }
        let process = command.output().map_err(|err| Error::IOError { err, file: String::new() })?;

        if !process.status.success() {
            return Err(Error::CompilerError {
                stderr: String::from_utf8_lossy(&process.stderr).to_string(),
                stdout: String::from_utf8_lossy(&process.stdout).to_string(),
            });
        }

        if exists(&output_file).map_or(false, |exists| !exists) {
            return Err(Error::CompilerError {
                stderr: "Output file was not created".to_string(),
                stdout: String::new(),
            });
        }

        Ok(output_file)
    }
}