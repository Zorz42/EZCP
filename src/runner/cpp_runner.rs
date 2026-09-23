use crate::progress::ScopedProgressBar;
use crate::runner::exec_runner::{RunResult, run_solution};
use crate::runner::gcc::{Gcc, canonicalize};
use crate::{Error, Result};
use indicatif::MultiProgress;
use log::{trace, warn};
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};

/// Kept low so that contention between the runs does not affect verdicts.
const MAX_CONCURRENT_SOLUTIONS: usize = 4;

/// Keeps scratch names unique between threads of one process.
static SCRATCH_FILES: AtomicUsize = AtomicUsize::new(0);

fn scratch_name(hash: u64) -> String {
    format!("p{hash}-{}-{}.tmp", std::process::id(), SCRATCH_FILES.fetch_add(1, Ordering::Relaxed))
}

/// Renames a finished file into place, so others see either none or all of it.
///
/// On Windows the rename fails while `destination` is open. Names are hashes of
/// the contents, so an existing `destination` is already the right file.
fn install(scratch: &Path, destination: &Path) -> Result<()> {
    std::fs::rename(scratch, destination).or_else(|err| {
        let _ = std::fs::remove_file(scratch);
        if destination.exists() { Ok(()) } else { Err(Error::io(destination)(err)) }
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProgramHandle(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskHandle(usize);

struct Task {
    program: ProgramHandle,
    input: Arc<str>,
    time_limit: i32,
    result: Option<RunResult>,
}

/// Compiles C++ programs into a cached build folder and runs them in parallel
/// under the timer.
pub struct CppRunner {
    gcc: Gcc,
    build_folder: PathBuf,
    timer: ProgramHandle,
    /// Executables, indexed by `ProgramHandle`.
    programs: Vec<PathBuf>,
    tasks: Vec<Task>,
    hash_to_handle: HashMap<u64, ProgramHandle>,
    /// What `clean_build_folder` keeps.
    necessary_files: HashSet<PathBuf>,
}

impl CppRunner {
    pub fn new(build_folder: &Path) -> Result<Self> {
        std::fs::create_dir_all(build_folder).map_err(Error::io(build_folder))?;
        let mut runner = Self {
            gcc: Gcc::new()?,
            build_folder: canonicalize(build_folder)?,
            timer: ProgramHandle(0),
            programs: Vec::new(),
            tasks: Vec::new(),
            hash_to_handle: HashMap::new(),
            necessary_files: HashSet::new(),
        };
        runner.timer = runner.add_program(include_str!("timer.cpp"))?;
        Ok(runner)
    }

    /// Compiles `source_code`, unless it is cached already.
    pub fn add_program(&mut self, source_code: &str) -> Result<ProgramHandle> {
        let hash = {
            let mut s = DefaultHasher::new();
            source_code.hash(&mut s);
            // The version stands in for the flags `Gcc::compile` passes.
            self.gcc.hash(&mut s);
            env!("CARGO_PKG_VERSION").hash(&mut s);
            s.finish()
        };

        if let Some(&handle) = self.hash_to_handle.get(&hash) {
            return Ok(handle);
        }

        let source_file = self.build_folder.join(format!("p{hash}.cpp"));
        let executable_file = Gcc::executable_path(&source_file)?;
        self.necessary_files.insert(source_file.clone());
        self.necessary_files.insert(executable_file.clone());

        // Written under scratch names and renamed into place, because several
        // processes (e.g. servers started by a judge) may share the build folder.
        // The source is always rewritten, to repair one truncated by an old version.
        let scratch_source = self.build_folder.join(scratch_name(hash));
        std::fs::write(&scratch_source, source_code).map_err(Error::io(&scratch_source))?;
        install(&scratch_source, &source_file)?;

        if !executable_file.exists() {
            trace!("Compiling: {}", executable_file.display());
            let scratch_executable = Gcc::executable_path(&self.build_folder.join(scratch_name(hash)))?;
            if let Err(err) = self.gcc.compile(&source_file, &scratch_executable) {
                let _ = std::fs::remove_file(&scratch_executable);
                return Err(err);
            }
            install(&scratch_executable, &executable_file)?;
        }

        // Only now, so a failed compile leaves no handle to a missing program.
        let handle = ProgramHandle(self.programs.len());
        self.programs.push(executable_file);
        self.hash_to_handle.insert(hash, handle);
        Ok(handle)
    }

    /// Queues a run of `program`; `time_limit` is in milliseconds of CPU time.
    pub fn add_task(&mut self, program: ProgramHandle, input: Arc<str>, time_limit: i32) -> TaskHandle {
        self.tasks.push(Task {
            program,
            input,
            time_limit,
            result: None,
        });
        TaskHandle(self.tasks.len() - 1)
    }

    pub fn clear_tasks(&mut self) {
        self.tasks.clear();
    }

    /// Hands over a finished task's input, so the runner no longer keeps it alive.
    pub fn take_input(&mut self, task: TaskHandle) -> Arc<str> {
        std::mem::take(&mut self.tasks[task.0].input)
    }

    /// # Panics
    /// Panics if the task has not run.
    #[allow(clippy::expect_used)]
    pub fn get_result(&self, task: TaskHandle) -> RunResult {
        self.tasks[task.0].result.clone().expect("Task result not available")
    }

    /// Runs every program on the same input.
    pub fn check_programs(&mut self, input: &str, programs: &[ProgramHandle], time_limit: i32) -> Result<Vec<RunResult>> {
        self.clear_tasks();
        let input: Arc<str> = Arc::from(input);
        let handles: Vec<_> = programs.iter().map(|&program| self.add_task(program, Arc::clone(&input), time_limit)).collect();
        self.run_tasks(None)?;
        let results = handles.into_iter().map(|handle| self.get_result(handle)).collect();
        self.clear_tasks();
        Ok(results)
    }

    /// Deletes the files in the build folder that belong to no added program.
    /// Call it only once every program has been added.
    pub fn clean_build_folder(&self) -> Result<()> {
        for entry in std::fs::read_dir(&self.build_folder).map_err(Error::io(&self.build_folder))? {
            let path = entry.map_err(Error::io(&self.build_folder))?.path();
            // A leftover only costs disk space, which is no reason to fail.
            if !self.necessary_files.contains(&path)
                && path.is_file()
                && let Err(err) = std::fs::remove_file(&path)
            {
                warn!("Could not remove {} from the build folder: {err}", path.display());
            }
        }
        Ok(())
    }

    pub fn run_tasks(&mut self, logger: Option<&MultiProgress>) -> Result<()> {
        let timer = &self.programs[self.timer.0];
        let num_threads = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get).min(MAX_CONCURRENT_SOLUTIONS);
        let progress_bar = logger.map(|logger| ScopedProgressBar::new(logger, self.tasks.len() as u64));
        let (sender, receiver) = mpsc::channel();
        let (mut next_task, mut running) = (0, 0);
        // Returning on the first error would leave running workers detached.
        let mut first_error = None;

        loop {
            // After a failure no new task is started; running ones are waited for.
            while running < num_threads && next_task < self.tasks.len() && first_error.is_none() {
                let task = &self.tasks[next_task];
                let (idx, executable, input, time_limit) = (next_task, self.programs[task.program.0].clone(), Arc::clone(&task.input), task.time_limit);
                let (timer, sender) = (timer.clone(), sender.clone());
                std::thread::spawn(move || {
                    let result = catch_unwind(AssertUnwindSafe(|| run_solution(&executable, input, time_limit, &timer))).unwrap_or_else(|_panic| {
                        Err(Error::TimerFailed {
                            details: format!("the worker thread for task {idx} panicked"),
                        })
                    });
                    let _ = sender.send((idx, result));
                });
                next_task += 1;
                running += 1;
            }

            if running == 0 {
                break;
            }
            // Cannot fail: this thread holds a sender.
            let Ok((idx, result)) = receiver.recv() else { break };
            running -= 1;
            match result {
                Ok(result) => {
                    trace!("Task {idx} finished with result: {result:?}");
                    self.tasks[idx].result = Some(result);
                }
                Err(err) => first_error = first_error.or(Some(err)),
            }
            if let Some(progress_bar) = &progress_bar {
                progress_bar.inc(1);
            }
        }

        first_error.map_or(Ok(()), Err)
    }
}
