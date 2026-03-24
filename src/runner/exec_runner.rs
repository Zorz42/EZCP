use crate::Error;
use crate::Result;
use log::trace;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The result of running a compiled program.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RunResult {
    /// Program finished successfully: (`elapsed_time_ms`, stdout)
    Ok(i32, String),
    /// Program exceeded the time limit.
    TimedOut,
    /// Program crashed or returned a non-zero exit code.
    Crashed,
}

impl RunResult {
    pub fn to_string(&self) -> String {
       match self {
           RunResult::Ok(_, _) => "OK".to_owned(),
           RunResult::TimedOut => "TLE".to_owned(),
           RunResult::Crashed => "RTE".to_owned(),
       } 
    }
}

/// Spawns the timer utility to execute and monitor a solution.
///
/// * `executable_file` - Path to the compiled C++ binary.
/// * `input_data` - Input to be sent via stdin.
/// * `time_limit` - Maximum CPU time in seconds.
/// * `timer_path` - Path to the pre-compiled `timer` utility.
pub fn run_solution(executable_file: &PathBuf, input_data: &str, time_limit: f32, timer_path: &Path) -> Result<RunResult> {
    let mut solution_process = Command::new(timer_path);
    solution_process.arg(executable_file);
    solution_process.arg(format!("{}", (time_limit * 1000.0) as i32));

    trace!("Running command: {solution_process:?}");
    // spawn the solution process
    let mut solution_process = solution_process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| Error::IOError { err, file: String::new() })?;

    if !input_data.is_empty() {
        let stdin = solution_process.stdin.as_mut().unwrap();
        // If the child exits early without reading stdin, writing can error with EPIPE.
        // Do not treat that as a crash of the child solution.
        let _ = stdin.write_all(input_data.as_bytes());
    }
    // Explicitly drop stdin to signal EOF to the child
    drop(solution_process.stdin.take());

    let output_result = solution_process.wait_with_output().map_err(|err| Error::IOError { err, file: String::new() })?;
    let return_code = output_result.status;

    if return_code.code() == Some(175) {
        trace!("Solution timed out with signal 175");
        return Ok(RunResult::TimedOut);
    }

    if !return_code.success() || return_code.code() != Some(0) {
        trace!("Solution crashed with return code: {:?}", return_code.code());
        return Ok(RunResult::Crashed);
    }

    let elapsed_time_ms = {
        // capture stderr from solution process
        let stderr_str = String::from_utf8_lossy(&output_result.stderr);
        // parse output from timer command (ignore trailing newlines/whitespace)
        let trimmed = stderr_str.trim();
        trimmed.parse::<i32>().unwrap()
    };
    trace!("Elapsed time from timer: {elapsed_time_ms} ms");

    let output = String::from_utf8_lossy(&output_result.stdout).into_owned();

    Ok(RunResult::Ok(elapsed_time_ms, output))
}
