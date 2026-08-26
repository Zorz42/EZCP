use crate::Error;
use crate::Result;
use crate::task::path_str;
use log::trace;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;

/// Marker the `timer` utility appends to stderr to report its verdict.
/// See `timer.cpp` for the protocol.
const RESULT_MARKER: &str = "__EZCP_RESULT__";

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
    pub fn to_display_string(&self) -> String {
        match self {
            Self::Ok(_, _) => "OK".to_owned(),
            Self::TimedOut => "TLE".to_owned(),
            Self::Crashed => "RTE".to_owned(),
        }
    }
}

/// Extracts `(verdict, elapsed_ms)` from the last result marker in the timer's stderr.
///
/// The last occurrence wins, so a solution that prints the marker itself cannot
/// spoof the verdict: the timer always writes after the solution has finished.
fn parse_result_marker(stderr: &str) -> Option<(&str, i32)> {
    let after_marker = &stderr[stderr.rfind(RESULT_MARKER)? + RESULT_MARKER.len()..];
    let mut fields = after_marker.lines().next().unwrap_or("").split_whitespace();
    let verdict = fields.next()?;
    let elapsed_ms = fields.next().and_then(|field| field.parse::<i64>().ok()).unwrap_or(0);
    Some((verdict, elapsed_ms.clamp(0, i64::from(i32::MAX)) as i32))
}

/// Spawns the timer utility to execute and monitor a solution.
///
/// * `executable_file` - Path to the compiled C++ binary.
/// * `input_data` - Input to be sent via stdin.
/// * `time_limit` - Maximum CPU time in milliseconds.
/// * `timer_path` - Path to the pre-compiled `timer` utility.
pub fn run_solution(executable_file: &Path, input_data: Arc<str>, time_limit: i32, timer_path: &Path) -> Result<RunResult> {
    let mut solution_process = Command::new(timer_path);
    solution_process.arg(executable_file);
    solution_process.arg(format!("{time_limit}"));

    trace!("Running command: {solution_process:?}");
    let mut solution_process = solution_process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| Error::IOError { err, file: path_str(timer_path) })?;

    // Feed stdin from a helper thread. Writing it inline would deadlock as soon
    // as the input outgrows the pipe buffer and the solution either stops
    // reading early or blocks writing output that nobody is draining yet.
    let stdin_writer = solution_process.stdin.take().map(|mut stdin| {
        std::thread::spawn(move || {
            // A broken pipe here just means the solution did not read all of its
            // input, which is perfectly normal for a partial solution.
            let _ = stdin.write_all(input_data.as_bytes());
            let _ = stdin.flush();
            // Dropping stdin signals EOF to the solution.
        })
    });

    let output_result = solution_process.wait_with_output().map_err(|err| Error::IOError { err, file: path_str(executable_file) })?;

    // The solution and the timer are both gone by now, so every read end of the
    // input pipe is closed and the writer cannot still be blocked.
    if let Some(stdin_writer) = stdin_writer {
        drop(stdin_writer.join());
    }

    let stderr_str = String::from_utf8_lossy(&output_result.stderr);
    let Some((verdict, elapsed_time_ms)) = parse_result_marker(&stderr_str) else {
        return Err(Error::TimerFailed {
            details: format!("timer did not report a result (exit code {:?})", output_result.status.code()),
        });
    };
    trace!("Timer reported verdict {verdict} after {elapsed_time_ms} ms");

    match verdict {
        "TLE" => Ok(RunResult::TimedOut),
        "RTE" => Ok(RunResult::Crashed),
        "OK" => {
            let output = String::from_utf8_lossy(&output_result.stdout).into_owned();
            // MinGW runs stdout in text mode, so every "\n" the solution printed
            // arrives as "\r\n". Undo that, otherwise the same solution produces
            // different output on Windows than on Unix.
            #[cfg(windows)]
            let output = output.replace("\r\n", "\n");
            Ok(RunResult::Ok(elapsed_time_ms, output))
        }
        _ => Err(Error::TimerFailed {
            details: format!("timer could not run the solution (verdict {verdict})"),
        }),
    }
}

#[cfg(test)]
mod parse_tests {
    use super::parse_result_marker;

    #[test]
    fn parses_verdict_and_time() {
        assert_eq!(parse_result_marker("\n__EZCP_RESULT__ OK 42\n"), Some(("OK", 42)));
    }

    #[test]
    fn ignores_output_the_solution_wrote_to_stderr() {
        let stderr = "debug line\nno trailing newline__EZCP_RESULT__ TLE 1234\n";
        assert_eq!(parse_result_marker(stderr), Some(("TLE", 1234)));
    }

    #[test]
    fn last_marker_wins_so_a_solution_cannot_spoof_the_verdict() {
        let stderr = "__EZCP_RESULT__ OK 0\nreal:\n__EZCP_RESULT__ RTE 7\n";
        assert_eq!(parse_result_marker(stderr), Some(("RTE", 7)));
    }

    #[test]
    fn tolerates_windows_line_endings() {
        assert_eq!(parse_result_marker("\r\n__EZCP_RESULT__ OK 5\r\n"), Some(("OK", 5)));
    }

    #[test]
    fn missing_marker_is_reported_as_missing() {
        assert_eq!(parse_result_marker("segmentation fault\n"), None);
    }

    #[test]
    fn missing_or_bogus_time_does_not_panic() {
        assert_eq!(parse_result_marker("__EZCP_RESULT__ OK\n"), Some(("OK", 0)));
        assert_eq!(parse_result_marker("__EZCP_RESULT__ OK nonsense\n"), Some(("OK", 0)));
        assert_eq!(parse_result_marker("__EZCP_RESULT__ OK 99999999999999\n"), Some(("OK", i32::MAX)));
        assert_eq!(parse_result_marker("__EZCP_RESULT__"), None);
    }
}
