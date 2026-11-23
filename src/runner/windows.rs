//! Limits and measures a solution through a job object.

use crate::runner::exec_runner::{Exit, Limits, Usage, poll_interval};
use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};
use std::sync::Once;
use std::time::Instant;
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows_sys::Win32::System::Diagnostics::Debug::{SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX, SetErrorMode};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOB_OBJECT_LIMIT_PROCESS_TIME, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, GetProcessTimes};

pub fn limit(command: &mut Command, _limits: Limits) {
    static ERROR_MODE: Once = Once::new();
    // Inherited by the solutions: a crash must not open a dialog and block the run.
    // SAFETY: only sets a flag of this process.
    ERROR_MODE.call_once(|| unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
    });
    command.creation_flags(CREATE_NO_WINDOW);
}

pub const fn is_out_of_memory(_err: &io::Error) -> bool {
    false
}

/// Kills the solution when closed, however EZCP ends: Windows does not kill children with their parent.
struct Job(HANDLE);

impl Job {
    /// Its CPU limit is a backstop; `wait` decides. Solutions start no processes, so assigning one
    /// after it started lets nothing escape.
    fn new(solution: HANDLE, limits: Limits) -> Option<Self> {
        // SAFETY: null asks for default security and no name.
        let job = Self(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) });
        if job.0.is_null() {
            return None;
        }

        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PROCESS_TIME;
        // In 100 ns ticks.
        info.BasicLimitInformation.PerProcessUserTimeLimit = ((limits.cpu_seconds() + 5) * 10_000_000) as i64;
        if let Some(cap) = limits.memory_cap_bytes() {
            info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_PROCESS_MEMORY;
            info.ProcessMemoryLimit = usize::try_from(cap).unwrap_or(usize::MAX);
        }
        // SAFETY: `info` is the struct the information class names, and outlives the call.
        unsafe {
            SetInformationJobObject(job.0, JobObjectExtendedLimitInformation, (&raw const info).cast(), size_of_val(&info) as u32);
            // Fails only if the solution has already ended.
            AssignProcessToJobObject(job.0, solution);
        }
        Some(job)
    }

    /// Peak committed memory.
    fn peak_memory_kb(&self) -> u64 {
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // SAFETY: as in `new`.
        let ok = unsafe { QueryInformationJobObject(self.0, JobObjectExtendedLimitInformation, (&raw mut info).cast(), size_of_val(&info) as u32, std::ptr::null_mut()) };
        if ok == 0 { 0 } else { info.PeakProcessMemoryUsed as u64 / 1024 }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: the handle is this job's own.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn cpu_time_ms(process: HANDLE) -> u64 {
    let (mut creation, mut exit, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: all four pointers are to locals that outlive the call.
    let ok = unsafe { GetProcessTimes(process, &raw mut creation, &raw mut exit, &raw mut kernel, &raw mut user) };
    // In 100 ns ticks.
    let ticks = |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    if ok == 0 { 0 } else { (ticks(kernel) + ticks(user)) / 10_000 }
}

pub fn wait(solution: &mut Child, limits: Limits, deadline: Instant) -> io::Result<Usage> {
    let process = solution.as_raw_handle();
    let job = Job::new(process, limits);
    let cpu_limit_ms = limits.cpu_seconds() * 1000;
    let start = Instant::now();
    let exit = loop {
        if let Some(status) = solution.try_wait()? {
            break if status.success() { Exit::Success } else { Exit::Failure };
        }
        if cpu_time_ms(process) > cpu_limit_ms || Instant::now() >= deadline {
            // Fails if it has just ended on its own.
            let _ = solution.kill();
            solution.wait()?;
            break Exit::OutOfTime;
        }
        std::thread::sleep(poll_interval(start.elapsed()));
    };
    Ok(Usage {
        exit,
        cpu_time_ms: cpu_time_ms(process),
        peak_memory_kb: job.as_ref().map_or(0, Job::peak_memory_kb),
    })
}
