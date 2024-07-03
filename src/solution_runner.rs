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
fn get_gcc_path() -> anyhow::Result<WindowsCompiler> {
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

    anyhow::bail!("g++ is not installed, specify the path to g++ with the GCC_PATH environment variable");
}

pub fn build_solution(source_file: &PathBuf, executable_file: &PathBuf) -> anyhow::Result<bool> {
    // if solution executable exists, check if it's up to date
    if executable_file.exists() {
        let solution_last_modified = std::fs::metadata(source_file)?.modified()?;
        let solution_exe_last_modified = std::fs::metadata(executable_file)?.modified()?;

        if solution_exe_last_modified > solution_last_modified {
            return Ok(false);
        }
    }

    #[cfg(windows)]
    {
        let gcc_path = get_gcc_path()?;
        let prev_working_dir = std::env::current_dir()?;

        let mut process = std::process::Command::new(gcc_path.get_path());

        if let WindowsCompiler::FullPath(gcc_path) = &gcc_path {
            let working_dir = std::path::Path::new(gcc_path).parent().ok_or_else(|| anyhow::anyhow!("Failed to get working directory"))?.to_path_buf();
            process.current_dir(working_dir);
        }

        // check if g++ is installed
        if std::process::Command::new(gcc_path.get_path()).arg("--version").output().is_err() {
            anyhow::bail!("g++ is not installed");
        }

        let executable_file = prev_working_dir.join(executable_file);
        let source_file = prev_working_dir.join(source_file);

        // invoke g++ to build solution
        let process = process.arg("-std=c++17").arg("-O2").arg("-o").arg(executable_file).arg(source_file).output()?;

        if !process.status.success() {
            anyhow::bail!(
                "Failed to build solution:\nstderr:\n{}\nstdout:\n{}\nstatus:{}\n",
                String::from_utf8_lossy(&process.stderr),
                String::from_utf8_lossy(&process.stdout),
                process.status
            );
        }
    }

    #[cfg(unix)]
    {
        // check if g++ is installed
        if std::process::Command::new("g++").arg("--version").output().is_err() {
            anyhow::bail!("g++ is not installed");
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
            anyhow::bail!(
                "Failed to build solution:\nstderr:\n{}\nstdout:\n{}\nstatus:{}\n",
                String::from_utf8_lossy(&process.stderr),
                String::from_utf8_lossy(&process.stdout),
                process.status
            );
        }
    }

    Ok(true)
}

pub fn run_solution(executable_file: &PathBuf, input_file: &PathBuf, output_file: &PathBuf, time_limit: f32, test_id: i32) -> anyhow::Result<f32> {
    // also time the solution
    let start_time = std::time::Instant::now();

    let working_dir = std::env::current_dir()?;

    let executable_file = working_dir.join(executable_file);
    let mut solution_process = std::process::Command::new(executable_file);

    #[cfg(windows)]
    {
        let gcc_path = get_gcc_path()?;
        if let WindowsCompiler::FullPath(gcc_path) = &gcc_path {
            let working_dir = std::path::Path::new(gcc_path).parent().ok_or_else(|| anyhow::anyhow!("Failed to get working directory"))?.to_path_buf();
            solution_process.current_dir(working_dir);
        }
    }

    let input_file = working_dir.join(input_file);
    let output_file = working_dir.join(output_file);

    // spawn the solution process
    let mut solution_process = solution_process.stdin(std::fs::File::open(input_file)?).stdout(std::fs::File::create(output_file)?).spawn()?;

    while solution_process.try_wait()?.is_none() {
        std::thread::sleep(std::time::Duration::from_millis(1));
        if start_time.elapsed().as_secs_f32() > time_limit {
            solution_process.kill()?;
            anyhow::bail!("Solution timed out on test {}", test_id);
        }
    }

    let solution_status = solution_process.wait()?;
    let elapsed_time = start_time.elapsed().as_secs_f32();

    if !solution_status.success() {
        anyhow::bail!("Solution failed on test {}", test_id);
    }

    Ok(elapsed_time)
}

// ignores whitespace
pub fn are_files_equal(file1: &PathBuf, file2: &PathBuf) -> anyhow::Result<bool> {
    let file1 = std::fs::read_to_string(file1)?;
    let file2 = std::fs::read_to_string(file2)?;

    let file1 = file1.split_whitespace().collect::<String>();
    let file2 = file2.split_whitespace().collect::<String>();

    Ok(file1 == file2)
}
