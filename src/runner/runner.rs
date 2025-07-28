use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use log::trace;
use crate::Error;
use crate::Result;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RunResult {
    Ok(i32, String), // elapsed time in milliseconds and output
    TimedOut,
    Crashed,
}

/// This function takes an executable file and runs it with the input file.
/// It writes the output to the output file, and returns the result of the test.
/// Build dir is needed, so that timer can be built if it's not already.
pub fn run_solution(executable_file: &PathBuf, input_data: &str, time_limit: f32, timer_path: &Path) -> Result<RunResult> {
    let mut solution_process = Command::new(timer_path);
    solution_process.arg(executable_file);
    solution_process.arg(format!("{}", (time_limit * 1000.0) as i32));

    trace!("Running command: {:?}", solution_process);
    // spawn the solution process
    let mut solution_process = solution_process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| Error::IOError { err, file: String::new() })?;

    if !input_data.is_empty() {
        let stdin = solution_process.stdin.as_mut().unwrap();
        let res = stdin.write_all(input_data.as_bytes());
        if res.is_err() {
            return Ok(RunResult::Crashed);
        }
    }

    let return_code = solution_process.wait().map_err(|err| Error::IOError { err, file: String::new() })?;

    if return_code.code() == Some(1) {
        return Ok(RunResult::TimedOut);
    }

    if !return_code.success() {
        return Ok(RunResult::Crashed);
    }

    let elapsed_time_ms = {
        // capture stderr from solution process
        let stderr = solution_process.stderr.as_mut().unwrap();
        let mut stderr_str = String::new();
        stderr.read_to_string(&mut stderr_str).map_err(|err| Error::IOError { err, file: String::new() })?;
        // parse output from timer command
        stderr_str.parse::<i32>().unwrap()
    };

    let output = {
        let stdout = solution_process.stdout.as_mut().unwrap();
        let mut output_str = String::new();
        stdout.read_to_string(&mut output_str).map_err(|err| Error::IOError { err, file: String::new() })?;
        output_str
    };

    if elapsed_time_ms as f32 * 0.001 > time_limit {
        return Ok(RunResult::TimedOut);
    }

    Ok(RunResult::Ok(elapsed_time_ms, output))
}