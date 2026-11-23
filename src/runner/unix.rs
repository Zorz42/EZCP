//! Limits a solution with `setrlimit` before it starts, and measures it with `wait4`.

use crate::runner::exec_runner::{Exit, Limits, Usage, poll_interval};
use std::io;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus};
use std::time::Instant;

pub fn limit(command: &mut Command, limits: Limits) {
    let cpu_seconds = limits.cpu_seconds();
    // macOS does not enforce an address space limit.
    let memory_cap = if cfg!(target_os = "linux") { limits.memory_cap_bytes() } else { None };
    #[cfg(target_os = "linux")]
    let parent = std::process::id() as libc::pid_t;

    // SAFETY: runs between fork and exec, so it only makes async-signal-safe calls.
    unsafe {
        command.pre_exec(move || {
            #[cfg(target_os = "linux")]
            {
                // Die with EZCP, which may already be gone. The signal follows the spawning thread,
                // which waits for the solution anyway.
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL as libc::c_ulong);
                if libc::getppid() != parent {
                    return Err(io::Error::from_raw_os_error(libc::ESRCH));
                }
            }

            // SIGXCPU at the soft limit, SIGKILL at the hard one.
            let cpu = libc::rlimit {
                rlim_cur: cpu_seconds,
                rlim_max: cpu_seconds + 5,
            };
            libc::setrlimit(libc::RLIMIT_CPU, &raw const cpu);

            // The soft limit as high as the hard one allows, and no higher than `cap`.
            let raise = |resource, cap: Option<u64>| {
                let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
                if libc::getrlimit(resource, &raw mut limit) == 0 {
                    let wanted = cap.map_or(limit.rlim_max, |cap| cap.min(limit.rlim_max));
                    if limit.rlim_cur != wanted {
                        limit.rlim_cur = wanted;
                        libc::setrlimit(resource, &raw const limit);
                    }
                }
            };
            // Solutions often recurse deeply.
            raise(libc::RLIMIT_STACK, None);
            raise(libc::RLIMIT_AS, memory_cap);
            Ok(())
        });
    }
}

pub fn is_out_of_memory(err: &io::Error) -> bool {
    err.raw_os_error() == Some(libc::ENOMEM)
}

#[allow(clippy::needless_pass_by_ref_mut, reason = "the Windows version needs it")]
pub fn wait(solution: &mut Child, _limits: Limits, deadline: Instant) -> io::Result<Usage> {
    let pid = solution.id() as libc::pid_t;
    let start = Instant::now();
    loop {
        if let Some(usage) = wait4(pid, libc::WNOHANG)? {
            return Ok(usage);
        }
        if Instant::now() >= deadline {
            // SAFETY: not reaped yet, so the pid is still the solution's.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
            return wait4(pid, 0)?.ok_or_else(|| io::Error::other("the solution was killed but did not end"));
        }
        std::thread::sleep(poll_interval(start.elapsed()));
    }
}

/// Reaps the solution, unlike `Child::wait`, with what it used. `None` if it is still running.
fn wait4(pid: libc::pid_t, options: libc::c_int) -> io::Result<Option<Usage>> {
    let mut status = 0;
    // SAFETY: plain data, which wait4 only writes to.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    loop {
        // SAFETY: both pointers are to locals that outlive the call.
        match unsafe { libc::wait4(pid, &raw mut status, options, &raw mut usage) } {
            0 => return Ok(None),
            -1 if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted => {}
            -1 => return Err(io::Error::last_os_error()),
            _ => break,
        }
    }

    let status = ExitStatus::from_raw(status);
    let exit = match status.signal() {
        // From the CPU limits or the wall-clock kill.
        Some(libc::SIGXCPU | libc::SIGKILL) => Exit::OutOfTime,
        None if status.success() => Exit::Success,
        _ => Exit::Failure,
    };
    let ms = |time: libc::timeval| time.tv_sec as u64 * 1000 + time.tv_usec as u64 / 1000;
    let peak_memory = usage.ru_maxrss as u64;
    Ok(Some(Usage {
        exit,
        cpu_time_ms: ms(usage.ru_utime) + ms(usage.ru_stime),
        // In bytes on macOS.
        peak_memory_kb: if cfg!(target_os = "macos") { peak_memory / 1024 } else { peak_memory },
    }))
}
