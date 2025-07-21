use std::path::{Path, PathBuf};
use crate::Error::{CompilerNotFound};
use crate::Result;

fn find_gcc() -> Result<PathBuf> {
    if let Ok(gcc_path) = std::env::var("GCC_PATH") {
        return Ok(PathBuf::from(gcc_path));
    }

    #[cfg(unix)]
    {
        // use which to find gcc in the PATH
        match which::which("g++") {
            Ok(path) => Ok(path),
            Err(_) => Err(CompilerNotFound),
        }
    }
    #[cfg(windows)]
    {
        let candidates = [
            "g++",
            "mingw32-g++",
            "x86_64-w64-mingw32-g++",
            "i686-w64-mingw32-g++",
            "c++",
            "cl.exe",
        ];

        for candidate in candidates {
            if let Ok(gcc_path) = which::which("g++") {
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
                let path = PathBuf::from(dir).join(candidate);
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

    pub fn compile(&self, source_file: &Path, output_file: &Path) -> std::io::Result<()> {
        let mut command = std::process::Command::new(&self.path);
        command.arg(source_file).arg("-o").arg(output_file);
        for flag in &self.flags {
            command.arg(flag);
        }
        command.status().map(|_| ())
    }
}