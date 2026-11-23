use crate::runner::sys;
use crate::{Error, Result};
use log::{debug, trace};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Output is held in memory, and a solution stuck printing can write gigabytes within its time limit.
const MAX_OUTPUT_BYTES: u64 = 256 << 20;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    /// CPU time.
    pub time_ms: i32,
    /// Peak memory.
    pub memory_kb: Option<u64>,
}

impl Limits {
    const fn time_ms_or_default(self) -> u64 {
        if self.time_ms > 0 { self.time_ms as u64 } else { 1000 }
    }

    /// Whole seconds, as `RLIMIT_CPU` takes; the exact limit is applied to the measured time.
    pub const fn cpu_seconds(self) -> u64 {
        self.time_ms_or_default().div_ceil(1000)
    }

    /// Far above the limit: EZCP compares the peak with the limit itself, and the cap only stops a
    /// runaway solution before it exhausts the machine.
    pub fn memory_cap_bytes(self) -> Option<u64> {
        self.memory_kb.map(|memory_kb| memory_kb.saturating_mul(2048).saturating_add(1 << 30))
    }

    /// Generous, so CPU time decides; it catches solutions that block.
    const fn wall_clock(self) -> Duration {
        Duration::from_millis(self.time_ms_or_default() * 2 + 2000)
    }
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
pub enum Exit {
    Success,
    Failure,
    /// Stopped by the CPU limit or the wall clock.
    OutOfTime,
}

/// How a solution's process ended and what it used, as `sys::wait` measures it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Usage {
    pub exit: Exit,
    pub cpu_time_ms: u64,
    pub peak_memory_kb: u64,
}

/// An eighth of the time so far: a short run is noticed soon, a long one not polled needlessly.
pub fn poll_interval(elapsed: Duration) -> Duration {
    (elapsed / 8).clamp(Duration::from_micros(50), Duration::from_millis(2))
}

/// `None` past [`MAX_OUTPUT_BYTES`]. Closing the pipe then kills a solution still writing (`SIGPIPE`).
fn read_output(stdout: impl Read) -> std::io::Result<Option<Vec<u8>>> {
    let mut output = Vec::new();
    stdout.take(MAX_OUTPUT_BYTES + 1).read_to_end(&mut output)?;
    Ok((output.len() as u64 <= MAX_OUTPUT_BYTES).then_some(output))
}

pub fn run_solution(executable_file: &Path, input_data: &str, limits: Limits) -> Result<RunResult> {
    let mut command = Command::new(executable_file);
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    sys::limit(&mut command, limits);
    trace!("Running command: {command:?}");
    let mut solution = match command.spawn() {
        Ok(solution) => solution,
        // Too large to even load under the memory cap.
        Err(err) if limits.memory_kb.is_some() && sys::is_out_of_memory(&err) => return Ok(RunResult::MemoryLimitExceeded),
        Err(err) => {
            return Err(Error::TimerFailed {
                details: format!("could not start {}: {err}", executable_file.display()),
            });
        }
    };
    let deadline = Instant::now() + limits.wall_clock();
    let (stdin, stdout) = (solution.stdin.take(), solution.stdout.take());

    // Both pipes at once: one filling up unserviced would deadlock.
    let (stdout, usage) = std::thread::scope(|scope| {
        if let Some(mut stdin) = stdin {
            // Fails if the solution does not read all of its input, which is fine.
            scope.spawn(move || drop(stdin.write_all(input_data.as_bytes())));
        }
        let stdout_reader = scope.spawn(move || stdout.map_or_else(|| Ok(Some(Vec::new())), read_output));
        // Killed on failure, or the threads would wait for it forever.
        let usage = sys::wait(&mut solution, limits, deadline).inspect_err(|_| drop(solution.kill()));
        let stdout = stdout_reader.join().unwrap_or_else(|_panic| Err(std::io::Error::other("the thread reading stdout panicked")));
        (stdout, usage)
    });

    let usage = usage.map_err(Error::io(executable_file))?;
    let stdout = stdout.map_err(Error::io(executable_file))?;
    trace!("Solution ended: {usage:?}");

    let Some(stdout) = stdout else {
        debug!("Solution wrote more than {MAX_OUTPUT_BYTES} bytes to stdout and was stopped");
        return Ok(RunResult::Crashed);
    };

    let time_ms = usage.cpu_time_ms.min(i32::MAX as u64) as i32;
    // A solution over the limit may well have finished, or crashed for lack of memory.
    let over_memory_limit = limits.memory_kb.is_some_and(|limit| usage.peak_memory_kb > limit);

    Ok(match usage.exit {
        Exit::OutOfTime => RunResult::TimedOut,
        _ if over_memory_limit => {
            trace!("Solution used {} kB of memory, more than the limit", usage.peak_memory_kb);
            RunResult::MemoryLimitExceeded
        }
        Exit::Failure => RunResult::Crashed,
        Exit::Success if limits.time_ms > 0 && time_ms > limits.time_ms => {
            trace!("Solution used {time_ms} ms of CPU time, more than the {} ms limit", limits.time_ms);
            RunResult::TimedOut
        }
        Exit::Success => {
            let output = String::from_utf8(stdout).unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned());
            // MinGW writes stdout in text mode.
            #[cfg(windows)]
            let output = output.replace("\r\n", "\n");
            RunResult::Ok {
                time_ms,
                memory_kb: usage.peak_memory_kb,
                output,
            }
        }
    })
}
