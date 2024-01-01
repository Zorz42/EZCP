use anyhow::{bail, Result};
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
    
    let possible_commands = [
        "g++",
        "c++",
    ];
    
    for command in possible_commands {
        if let Ok(gcc_path) = std::process::Command::new(command).arg("--version").output() {
            if gcc_path.status.success() {
                return Ok(WindowsCompiler::Command(PathBuf::from(command)));
            }
        }
    }
    
    let possible_paths = [
        "C:\\MinGW\\bin\\c++.exe",
    ];
    
    for path in possible_paths {
        if PathBuf::from(path).exists() {
            return Ok(WindowsCompiler::FullPath(PathBuf::from(path)));
        }
    }
    
    bail!("g++ is not installed, specify the path to g++ with the GCC_PATH environment variable");
}

pub fn build_solution(source_file: &PathBuf, executable_file: &PathBuf) -> Result<bool> {
    // if solution executable exists, check if it's up to date
    if executable_file.exists() {
        let solution_last_modified = std::fs::metadata(source_file)?.modified()?;
        let solution_exe_last_modified = std::fs::metadata(executable_file)?.modified()?;

        if solution_exe_last_modified > solution_last_modified {
            return Ok(false);
        }
    }

    #[cfg(windows)] {
        let gcc_path = get_gcc_path()?;
        let prev_working_dir = std::env::current_dir()?;
        if let WindowsCompiler::FullPath(gcc_path) = &gcc_path {
            let working_dir = std::path::Path::new(gcc_path).parent().ok_or_else(|| anyhow::anyhow!("Failed to get working directory"))?.to_path_buf();
            std::env::set_current_dir(working_dir)?;
        }
        
        // check if g++ is installed
        if std::process::Command::new(gcc_path.get_path()).arg("--version").output().is_err() {
            bail!("g++ is not installed");
        }
        
        let executable_file = prev_working_dir.join(executable_file);
        let source_file = prev_working_dir.join(source_file);
        
        // invoke g++ to build solution
        let process = std::process::Command::new(gcc_path.get_path())
            .arg("-std=c++17")
            .arg("-O2")
            .arg("-o")
            .arg(executable_file)
            .arg(source_file)
            .output()?;

        if !process.status.success() {
            bail!("Failed to build solution:\nstderr:\n{}\nstdout:\n{}\n", String::from_utf8_lossy(&process.stderr), String::from_utf8_lossy(&process.stdout));
        }
        
        std::env::set_current_dir(&prev_working_dir)?;
    }

    #[cfg(unix)] {
        // check if g++ is installed
        if std::process::Command::new("g++").arg("--version").output().is_err() {
            bail!("g++ is not installed");
        }

        // invoke g++ to build solution
        let process = std::process::Command::new("g++")
            .arg("-std=c++20")
            .arg("-O2")
            .arg("-o")
            .arg(executable_file)
            .arg(source_file)
            .output()?;

        if !process.status.success() {
            bail!("Failed to build solution:\nstderr:\n{}\nstdout:\n{}\n", String::from_utf8_lossy(&process.stderr), String::from_utf8_lossy(&process.stdout));
        }
    }

    Ok(true)
}

pub fn run_solution(executable_file: &PathBuf, input_file: &PathBuf, output_file: &PathBuf, time_limit: f32, test_id: i32) -> Result<f32> {
    // also time the solution
    let start_time = std::time::Instant::now();

    let prev_working_dir = std::env::current_dir()?;
    
    #[cfg(windows)]
    let gcc_path = get_gcc_path()?;
    #[cfg(windows)]
    let working_dir = std::path::Path::new(&gcc_path.get_path()).parent().ok_or_else(|| anyhow::anyhow!("Failed to get working directory"))?.to_path_buf();
    #[cfg(windows)]
    if let WindowsCompiler::FullPath(_) = gcc_path {
        std::env::set_current_dir(working_dir)?;
    }
    
    let input_file = prev_working_dir.join(input_file);
    let output_file = prev_working_dir.join(output_file);
    let executable_file = prev_working_dir.join(executable_file);
    
    // spawn the solution process
    let mut solution_process = std::process::Command::new(executable_file)
        .stdin(std::fs::File::open(input_file)?)
        .stdout(std::fs::File::create(output_file)?)
        .spawn()?;

    while solution_process.try_wait()?.is_none() {
        std::thread::sleep(std::time::Duration::from_millis(1));
        if start_time.elapsed().as_secs_f32() > time_limit {
            solution_process.kill()?;
            bail!("Solution timed out on test {}", test_id);
        }
    }

    let solution_status = solution_process.wait()?;
    let elapsed_time = start_time.elapsed().as_secs_f32();
    
    if !solution_status.success() {
        bail!("Solution failed on test {}", test_id);
    }
    
    #[cfg(windows)]
    std::env::set_current_dir(&prev_working_dir)?;

    Ok(elapsed_time)
}

// ignores whitespace
pub fn are_files_equal(file1: &PathBuf, file2: &PathBuf) -> Result<bool> {
    let file1 = std::fs::read_to_string(file1)?;
    let file2 = std::fs::read_to_string(file2)?;

    let file1 = file1.split_whitespace().collect::<String>();
    let file2 = file2.split_whitespace().collect::<String>();

    Ok(file1 == file2)
}
