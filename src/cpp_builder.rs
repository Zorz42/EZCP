use std::path::PathBuf;
use crate::Error;
use crate::logger::Logger;

#[cfg(windows)]
pub enum WindowsCompiler {
    FullPath(PathBuf),
    Command(PathBuf),
}

#[cfg(windows)]
impl WindowsCompiler {
    pub fn get_path(&self) -> PathBuf {
        match self {
            Self::FullPath(path) => path.clone(),
            Self::Command(command) => command.clone(),
        }
    }
}

#[cfg(windows)]
pub fn get_gcc_path() -> crate::Result<WindowsCompiler> {
    if let Ok(gcc_path) = std::env::var("GCC_PATH") {
        return Ok(WindowsCompiler::FullPath(PathBuf::from(gcc_path)));
    }
    let possible_commands = ["g++", "c++"];
    for command in possible_commands {
        if let Ok(gcc_path) = std::process::Command::new(command).arg("--version").output() {
            if gcc_path.status.success() {
                return Ok(WindowsCompiler::Command(PathBuf::from(command)));
            }
        }
    }

    let possible_paths = ["C:\\MinGW\\bin\\c++.exe"];
    for path in possible_paths {
        if PathBuf::from(path).exists() {
            return Ok(WindowsCompiler::FullPath(PathBuf::from(path)));
        }
    }

    Err(Error::CompilerNotFoundWindows)
}

/// The only job of this function is to build the solution.
/// It takes a c++ source file and produces an executable file.
/// It returns true if the executable was built and false if it was up to date.
pub fn build_solution(source_file: &PathBuf, executable_file: &PathBuf, logger: &Logger) -> crate::Result<bool> {
    // if solution executable exists, check if it's up to date
    if executable_file.exists() {
        let executable_file_str1 = executable_file.to_str().unwrap_or("???").to_owned();
        let executable_file_str2 = executable_file_str1.clone();
        let executable_file_str3 = executable_file_str1.clone();
        let executable_file_str4 = executable_file_str1.clone();
        let solution_last_modified = std::fs::metadata(source_file).map_err(|err| Error::IOError { err, file: executable_file_str1 })?.modified().map_err(|err| Error::IOError { err, file: executable_file_str2 })?;
        let solution_exe_last_modified = std::fs::metadata(executable_file)
            .map_err(|err| Error::IOError { err, file: executable_file_str3 })?
            .modified()
            .map_err(|err| Error::IOError { err, file: executable_file_str4 })?;

        if solution_exe_last_modified > solution_last_modified {
            return Ok(false);
        }
    }
    
    logger.logln(format!("Building file: {}", source_file.display()));

    #[cfg(windows)]
    {
        let gcc_path = get_gcc_path()?;
        let prev_working_dir = std::env::current_dir().map_err(|err| Error::IOError { err, file: "Error".to_owned() })?;

        let mut process = std::process::Command::new(gcc_path.get_path());

        if let WindowsCompiler::FullPath(gcc_path) = &gcc_path {
            let working_dir = std::path::Path::new(gcc_path).parent().unwrap_or_else(|| std::path::Path::new("/")).to_path_buf();
            process.current_dir(working_dir);
        }

        // check if g++ is installed
        if std::process::Command::new(gcc_path.get_path()).arg("--version").output().is_err() {
            return Err(Error::CompilerNotFoundWindows);
        }

        let executable_file = prev_working_dir.join(executable_file);
        let source_file = prev_working_dir.join(source_file);

        // invoke g++ to build solution
        let process = process
            .arg("-std=c++17")
            .arg("-O2")
            .arg("-o")
            .arg(executable_file)
            .arg(source_file)
            .output()
            .map_err(|err| Error::IOError { err, file: "Error".to_owned() })?;

        if !process.status.success() {
            return Err(Error::CompilerError {
                stderr: String::from_utf8_lossy(&process.stderr).to_string(),
                stdout: String::from_utf8_lossy(&process.stdout).to_string(),
            });
        }
    }

    #[cfg(unix)]
    {
        // check if g++ is installed
        if std::process::Command::new("g++").arg("--version").output().is_err() {
            return Err(Error::CompilerNotFound);
        }

        // invoke g++ to build solution
        let process = std::process::Command::new("g++")
            .arg("-std=c++20")
            .arg("-O2")
            .arg("-o")
            .arg(executable_file)
            .arg(source_file)
            .output()
            .map_err(|err| Error::IOError { err, file: String::new() })?;

        if !process.status.success() {
            return Err(Error::CompilerError {
                stderr: String::from_utf8_lossy(&process.stderr).to_string(),
                stdout: String::from_utf8_lossy(&process.stdout).to_string(),
            });
        }
    }

    Ok(true)
}