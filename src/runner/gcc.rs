use std::path::{Path, PathBuf};
use log::debug;
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
    
    /// Transforms the output file path based on the source file and the specified output file.
    pub fn transform_output_file(source_file: &PathBuf, output_file: Option<&PathBuf>) -> Result<PathBuf> {
        let mut output_file = output_file.map_or(source_file, |p| p).to_owned();
        #[cfg(windows)]
        {
            output_file.set_extension("exe");
        }
        #[cfg(unix)]
        {
            output_file.set_extension("");
        }

        // create output file and its parent directories if they do not exist
        if let Some(parent) = output_file.parent() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|err| Error::IOError { err, file: parent.to_string_lossy().to_string() })?;
        }

        let output_existed = output_file.exists();
        if !output_file.exists() {
            std::fs::File::create(&output_file).map_err(|err| Error::IOError { err, file: output_file.to_string_lossy().to_string() })?;
        }

        // convert to absolute path; use dunce to normalize UNC on Windows
        let output_file = {
            #[cfg(windows)]
            { dunce::canonicalize(&output_file) }
            #[cfg(unix)]
            { std::fs::canonicalize(&output_file) }
        }.map_err(|err| Error::IOError { err, file: output_file.to_string_lossy().to_string() })?;


        if !output_existed {
            std::fs::remove_file(&output_file).map_err(|err| Error::IOError { err, file: output_file.to_string_lossy().to_string() })?;
        }

        Ok(output_file)
    }

    /// Calls `gcc` to compile the source file.
    /// If `output_file` is None, it will use the source file name with an appropriate extension.
    pub fn compile(&self, source_file: &Path, output_file: Option<&PathBuf>) -> Result<PathBuf> {
        // transform the path to absolute path; use dunce on Windows to avoid UNC (\\?\) paths
        let source_file = {
            #[cfg(windows)]
            { dunce::canonicalize(source_file) }
            #[cfg(unix)]
            { std::fs::canonicalize(source_file) }
        }.map_err(|err| Error::IOError { err, file: source_file.to_string_lossy().to_string() })?;

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
        }

        command.arg(source_file).arg("-o").arg(&output_file);
        // Do not override current_dir; pass absolute paths instead

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