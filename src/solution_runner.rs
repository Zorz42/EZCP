use anyhow::{bail, Result};
use std::path::PathBuf;

pub fn build_solution(source_file: &PathBuf, executable_file: &PathBuf) -> Result<bool> {
    // if solution executable exists, check if it's up to date
    if executable_file.exists() {
        let solution_last_modified = std::fs::metadata(source_file)?.modified()?;
        let solution_exe_last_modified = std::fs::metadata(executable_file)?.modified()?;

        if solution_exe_last_modified > solution_last_modified {
            return Ok(false);
        }
    }

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
        bail!("Failed to build solution");
    }

    Ok(true)
}

pub fn run_solution(executable_file: &PathBuf, input_file: &PathBuf, output_file: &PathBuf, time_limit: f32, test_id: i32) -> Result<f32> {
    // also time the solution
    let start_time = std::time::Instant::now();

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
