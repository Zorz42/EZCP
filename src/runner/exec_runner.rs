use crate::{Error, Result};
use log::{debug, trace};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// See `timer.cpp`.
const RESULT_MARKER: &str = "__EZCP_RESULT__";

/// Output is held in memory, and a solution stuck printing can write gigabytes within its time limit.
const MAX_OUTPUT_BYTES: u64 = 256 << 20;

/// Only the end of stderr is kept: the marker follows any amount of the solution's debug output.
const STDERR_TAIL_BYTES: usize = 64 << 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    /// CPU time.
    pub time_ms: i32,
    /// Peak memory.
    pub memory_kb: Option<u64>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RunResult {
    Ok {
        time_ms: i32,
        memory_kb: u64,
        output: String,
    },
    TimedOut,
    MemoryLimitExceeded,
    /// Also a non-zero exit code or more than [`MAX_OUTPUT_BYTES`] of output.
    Crashed,
}

impl RunResult {
    /// The output, or the error for the official solution failing.
    pub fn official_output(self, test_path: &str, gen_id: usize) -> Result<String> {
        let test_path = test_path.to_owned();
        match self {
            Self::Ok { output, .. } => Ok(output),
            Self::TimedOut => Err(Error::SolutionTimedOut { test_path, gen_id }),
            Self::MemoryLimitExceeded => Err(Error::SolutionOutOfMemory { test_path, gen_id }),
            Self::Crashed => Err(Error::SolutionCrash { test_path, gen_id }),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TimerReport<'stderr> {
    pub verdict: &'stderr str,
    pub time_ms: i32,
    pub memory_kb: u64,
}

/// The last marker, which the timer writes after the solution exits, so the solution cannot spoof it.
pub fn parse_result_marker(stderr: &str) -> Option<TimerReport<'_>> {
    let after_marker = &stderr[stderr.rfind(RESULT_MARKER)? + RESULT_MARKER.len()..];
    let mut fields = after_marker.lines().next().unwrap_or("").split_whitespace();
    let verdict = fields.next()?;
    let mut number = || fields.next().and_then(|field| field.parse::<i64>().ok()).unwrap_or(0).max(0);
    let (time_ms, memory_kb) = (number(), number());
    Some(TimerReport {
        verdict,
        time_ms: time_ms.min(i64::from(i32::MAX)) as i32,
        memory_kb: memory_kb as u64,
    })
}

/// `None` past [`MAX_OUTPUT_BYTES`]. Closing the pipe then kills a solution still writing (`SIGPIPE`).
fn read_output(stdout: impl Read) -> std::io::Result<Option<Vec<u8>>> {
    let mut output = Vec::new();
    stdout.take(MAX_OUTPUT_BYTES + 1).read_to_end(&mut output)?;
    Ok((output.len() as u64 <= MAX_OUTPUT_BYTES).then_some(output))
}

/// Keeps at least the last [`STDERR_TAIL_BYTES`].
fn read_tail(mut stderr: impl Read) -> std::io::Result<Vec<u8>> {
    let mut tail = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        match stderr.read(&mut chunk) {
            Ok(0) => return Ok(tail),
            Ok(read) => {
                tail.extend_from_slice(&chunk[..read]);
                if tail.len() > 2 * STDERR_TAIL_BYTES {
                    tail.drain(..tail.len() - STDERR_TAIL_BYTES);
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }
}

pub fn run_solution(executable_file: &Path, input_data: &str, limits: Limits, timer_path: &Path) -> Result<RunResult> {
    let mut command = Command::new(timer_path);
    command
        .arg(executable_file)
        .arg(limits.time_ms.to_string())
        .arg(limits.memory_kb.unwrap_or(0).to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    trace!("Running command: {command:?}");
    let mut solution_process = command.spawn().map_err(Error::io(timer_path))?;
    let (stdin, stdout, stderr) = (solution_process.stdin.take(), solution_process.stdout.take(), solution_process.stderr.take());

    // All three pipes at once: one filling up unserviced would deadlock.
    let (stdout, stderr, status) = std::thread::scope(|scope| {
        let stdin_writer = stdin.map(|mut stdin| {
            scope.spawn(move || {
                // Fails if the solution does not read all of its input, which is fine.
                let _ = stdin.write_all(input_data.as_bytes());
                let _ = stdin.flush();
            })
        });
        let stdout_reader = scope.spawn(move || stdout.map_or_else(|| Ok(Some(Vec::new())), read_output));
        let stderr = stderr.map_or_else(|| Ok(Vec::new()), read_tail);
        let status = solution_process.wait();

        // Both processes have exited, so the writer is not blocked.
        if let Some(stdin_writer) = stdin_writer {
            drop(stdin_writer.join());
        }
        let stdout = stdout_reader.join().unwrap_or_else(|_panic| Err(std::io::Error::other("the thread reading stdout panicked")));
        (stdout, stderr, status)
    });

    let stdout = stdout.map_err(Error::io(executable_file))?;
    let stderr = stderr.map_err(Error::io(executable_file))?;
    let status = status.map_err(Error::io(executable_file))?;

    let stderr_str = String::from_utf8_lossy(&stderr);
    let Some(report) = parse_result_marker(&stderr_str) else {
        return Err(Error::TimerFailed {
            details: format!("timer did not report a result (exit code {:?})", status.code()),
        });
    };
    trace!("Timer reported {report:?}");

    // The timer then says RTE (SIGPIPE), or TLE on Windows.
    let Some(stdout) = stdout else {
        debug!("Solution wrote more than {MAX_OUTPUT_BYTES} bytes to stdout and was stopped");
        return Ok(RunResult::Crashed);
    };

    // A solution over the limit may well have finished, or crashed for lack of memory.
    let over_memory_limit = limits.memory_kb.is_some_and(|limit| report.memory_kb > limit);

    match report.verdict {
        "TLE" => Ok(RunResult::TimedOut),
        "MLE" => Ok(RunResult::MemoryLimitExceeded),
        "OK" | "RTE" if over_memory_limit => {
            trace!("Solution used {} kB of memory, more than the limit", report.memory_kb);
            Ok(RunResult::MemoryLimitExceeded)
        }
        "RTE" => Ok(RunResult::Crashed),
        "OK" => {
            // The timer enforces the limit rounded up to whole seconds.
            if limits.time_ms > 0 && report.time_ms > limits.time_ms {
                trace!("Solution used {} ms of CPU time, more than the {} ms limit", report.time_ms, limits.time_ms);
                return Ok(RunResult::TimedOut);
            }

            let output = String::from_utf8(stdout).unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned());
            // MinGW writes stdout in text mode.
            #[cfg(windows)]
            let output = output.replace("\r\n", "\n");
            Ok(RunResult::Ok {
                time_ms: report.time_ms,
                memory_kb: report.memory_kb,
                output,
            })
        }
        verdict => Err(Error::TimerFailed {
            details: format!("timer could not run the solution (verdict {verdict})"),
        }),
    }
}
