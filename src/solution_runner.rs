use crate::{Error, Result};
use std::path::PathBuf;

#[cfg(windows)]
enum WindowsCompiler {
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
fn get_gcc_path() -> Result<WindowsCompiler> {
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

pub fn build_solution(source_file: &PathBuf, executable_file: &PathBuf) -> Result<bool> {
    // if solution executable exists, check if it's up to date
    if executable_file.exists() {
        let solution_last_modified = std::fs::metadata(source_file).map_err(|err| Error::IOError { err })?.modified().map_err(|err| Error::IOError { err })?;
        let solution_exe_last_modified = std::fs::metadata(executable_file)
            .map_err(|err| Error::IOError { err })?
            .modified()
            .map_err(|err| Error::IOError { err })?;

        if solution_exe_last_modified > solution_last_modified {
            return Ok(false);
        }
    }

    #[cfg(windows)]
    {
        let gcc_path = get_gcc_path()?;
        let prev_working_dir = std::env::current_dir().map_err(|err| Error::IOError { err })?;

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
            .map_err(|err| Error::IOError { err })?;

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
            return Err(Error::CompilerNotFoundUnix);
        }

        // invoke g++ to build solution
        let process = std::process::Command::new("g++")
            .arg("-std=c++20")
            .arg("-O2")
            .arg("-o")
            .arg(executable_file)
            .arg(source_file)
            .output()
            .map_err(|err| Error::IOError { err })?;

        if !process.status.success() {
            return Err(Error::CompilerError {
                stderr: String::from_utf8_lossy(&process.stderr).to_string(),
                stdout: String::from_utf8_lossy(&process.stdout).to_string(),
            });
        }
    }

    Ok(true)
}

pub enum TestResult {
    Ok(f32), // elapsed time
    TimedOut,
    Crashed,
}

pub fn run_solution(executable_file: &PathBuf, input_file: &PathBuf, output_file: &PathBuf, time_limit: f32) -> Result<TestResult> {
    // also time the solution
    let start_time = std::time::Instant::now();

    let working_dir = std::env::current_dir().map_err(|err| Error::IOError { err })?;

    let executable_file = working_dir.join(executable_file);
    let mut solution_process = std::process::Command::new(executable_file);

    #[cfg(windows)]
    {
        let gcc_path = get_gcc_path()?;
        if let WindowsCompiler::FullPath(gcc_path) = &gcc_path {
            let working_dir = std::path::Path::new(gcc_path).parent().unwrap_or_else(|| std::path::Path::new("/")).to_path_buf();
            solution_process.current_dir(working_dir);
        }
    }

    let input_file = working_dir.join(input_file);
    let output_file = working_dir.join(output_file);

    // spawn the solution process
    let mut solution_process = solution_process
        .stdin(std::fs::File::open(input_file).map_err(|err| Error::IOError { err })?)
        .stdout(std::fs::File::create(output_file).map_err(|err| Error::IOError { err })?)
        .spawn()
        .map_err(|err| Error::IOError { err })?;

    while solution_process.try_wait().map_err(|err| Error::IOError { err })?.is_none() {
        std::thread::sleep(std::time::Duration::from_millis(1));
        if start_time.elapsed().as_secs_f32() > time_limit {
            solution_process.kill().map_err(|err| Error::IOError { err })?;
            return Ok(TestResult::TimedOut);
        }
    }

    let solution_status = solution_process.wait().map_err(|err| Error::IOError { err })?;
    let elapsed_time = start_time.elapsed().as_secs_f32();

    if !solution_status.success() {
        return Ok(TestResult::Crashed);
    }

    Ok(TestResult::Ok(elapsed_time))
}

// ignores whitespace
pub fn are_files_equal(file1: &PathBuf, file2: &PathBuf) -> Result<bool> {
    let file1 = std::fs::read_to_string(file1).map_err(|err| Error::IOError { err })?;
    let file2 = std::fs::read_to_string(file2).map_err(|err| Error::IOError { err })?;

    let file1 = file1.split_whitespace().collect::<String>();
    let file2 = file2.split_whitespace().collect::<String>();

    Ok(file1 == file2)
}
