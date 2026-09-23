use crate::Error::IOError;
use crate::progress::ScopedProgressBar;
use crate::runner::exec_runner::{RunResult, run_solution};
use crate::runner::gcc::{Gcc, GccOptimization, GccStandard, canonicalize};
use crate::task::path_str;
use crate::{Error, Result};
use indicatif::MultiProgress;
use log::{trace, warn};
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{JoinHandle, spawn};
use std::time::Duration;

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
        if destination.exists() { Ok(()) } else { Err(IOError { err, file: path_str(destination) }) }
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProgramHandle {
    pub(crate) id: usize,
}

struct Task {
    program: ProgramHandle,
    input: Arc<str>,
    time_limit: i32,
    result: Option<RunResult>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskHandle {
    pub(crate) id: usize,
}

/// Compiles C++ programs into a cached build folder and runs them in parallel
/// under the timer.
pub struct CppRunner {
    gcc: Gcc,
    build_folder: PathBuf,
    timer: ProgramHandle,
    /// Executables, indexed by `ProgramHandle::id`.
    programs: Vec<PathBuf>,
    tasks: Vec<Task>,
    hash_to_handle: HashMap<u64, ProgramHandle>,
    /// What `clean_build_folder` keeps.
    necessary_files: HashSet<PathBuf>,
}

impl CppRunner {
    pub fn new(build_folder: &Path) -> Result<Self> {
        trace!("Creating CppRunner with build folder: {}", build_folder.to_string_lossy());
        if !build_folder.exists() {
            trace!("Build folder does not exist, creating: {}", build_folder.to_string_lossy());
            std::fs::create_dir_all(build_folder).map_err(|err| IOError {
                err,
                file: build_folder.to_string_lossy().to_string(),
            })?;
        }
        let mut gcc = Gcc::new()?;
        gcc.standard = Some(GccStandard::Cpp17);
        gcc.optimization = Some(GccOptimization::Level2);
        let build_folder = canonicalize(build_folder)?;
        let mut res = Self {
            gcc,
            build_folder,
            timer: ProgramHandle { id: 0 },
            programs: Vec::new(),
            tasks: Vec::new(),
            hash_to_handle: HashMap::new(),
            necessary_files: HashSet::new(),
        };

        trace!("Building timer program");
        let timer_source = include_str!("timer.cpp");
        res.timer = res.add_program(timer_source)?;

        Ok(res)
    }

    /// Compiles `source_code`, unless it is cached already.
    pub fn add_program(&mut self, source_code: &str) -> Result<ProgramHandle> {
        trace!("Adding program with source code: {source_code}");
        let handle = ProgramHandle { id: self.programs.len() };
        let hash = {
            let mut s = DefaultHasher::new();
            source_code.hash(&mut s);
            // The version stands in for the flags `Gcc::compile` adds by itself.
            self.gcc.hash(&mut s);
            env!("CARGO_PKG_VERSION").hash(&mut s);
            s.finish()
        };

        if let Some(existing_handle) = self.hash_to_handle.get(&hash) {
            trace!("Program already exists with id: {}", existing_handle.id);
            return Ok(*existing_handle);
        }

        let source_file = self.build_folder.join(format!("p{hash}.cpp"));
        let executable_file = Gcc::transform_output_file(&source_file, None)?;

        self.necessary_files.insert(source_file.clone());
        self.necessary_files.insert(executable_file.clone());

        // Written under scratch names and renamed into place, because several
        // processes (e.g. servers started by a judge) may share the build folder.
        // The source is always rewritten, to repair one truncated by an old version.
        let scratch_source = self.build_folder.join(scratch_name(hash));
        std::fs::write(&scratch_source, source_code).map_err(|err| IOError { err, file: path_str(&scratch_source) })?;
        install(&scratch_source, &source_file)?;

        if !executable_file.exists() {
            trace!("Compiling: {}", executable_file.to_string_lossy());
            let scratch_executable = Gcc::transform_output_file(&self.build_folder.join(scratch_name(hash)), None)?;
            if let Err(err) = self.gcc.compile(&source_file, Some(&scratch_executable)) {
                let _ = std::fs::remove_file(&scratch_executable);
                return Err(err);
            }
            install(&scratch_executable, &executable_file)?;
        }

        // Only now, so a failed compile leaves no handle to a missing program.
        self.programs.push(executable_file);
        self.hash_to_handle.insert(hash, handle);
        Ok(handle)
    }

    /// Queues a run of `program`; `time_limit` is in milliseconds of CPU time.
    pub fn add_task(&mut self, program: ProgramHandle, input: Arc<str>, time_limit: i32) -> TaskHandle {
        trace!("Adding task for program id: {}, time limit: {}", program.id, time_limit);
        let handle = TaskHandle { id: self.tasks.len() };
        self.tasks.push(Task {
            program,
            input,
            time_limit,
            result: None,
        });
        handle
    }

    pub fn clear_tasks(&mut self) {
        self.tasks.clear();
    }

    /// Hands over a finished task's input, so the runner no longer keeps it alive.
    pub fn take_input(&mut self, task_handle: TaskHandle) -> Arc<str> {
        std::mem::take(&mut self.tasks[task_handle.id].input)
    }

    /// # Panics
    /// Panics if the task has not run.
    #[allow(clippy::expect_used)]
    pub fn get_result(&self, task_handle: TaskHandle) -> RunResult {
        self.tasks[task_handle.id].result.clone().expect("Task result not available")
    }

    /// Runs every program on the same input.
    pub fn check_programs(&mut self, input: &str, programs: &[ProgramHandle], time_limit: i32) -> Result<Vec<RunResult>> {
        self.clear_tasks();
        let input: Arc<str> = Arc::from(input);
        let mut handles = Vec::new();
        for &program in programs {
            handles.push(self.add_task(program, Arc::clone(&input), time_limit));
        }
        self.run_tasks(None)?;
        let mut results = Vec::new();
        for handle in handles {
            results.push(self.get_result(handle));
        }
        self.clear_tasks();
        Ok(results)
    }

    /// Deletes the files in the build folder that belong to no added program.
    /// Call it only once every program has been added.
    pub fn clean_build_folder(&self) -> Result<()> {
        trace!("Cleaning build folder: {}", self.build_folder.to_string_lossy());

        let entries = std::fs::read_dir(&self.build_folder).map_err(|err| IOError {
            err,
            file: path_str(&self.build_folder),
        })?;
        for entry in entries {
            let entry = entry.map_err(|err| IOError {
                err,
                file: path_str(&self.build_folder),
            })?;
            let path = entry.path();
            if !self.necessary_files.contains(&path) && path.is_file() {
                // A leftover only costs disk space, which is no reason to fail.
                if let Err(err) = std::fs::remove_file(&path) {
                    warn!("Could not remove {} from the build folder: {err}", path_str(&path));
                }
            }
        }
        Ok(())
    }

    pub fn run_tasks(&mut self, logger: Option<&MultiProgress>) -> Result<()> {
        let timer_path = self.programs[self.timer.id].clone();

        let num_threads = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get).min(MAX_CONCURRENT_SOLUTIONS);
        let mut threads: Vec<(JoinHandle<Result<RunResult>>, usize)> = Vec::new();

        let mut next_task = 0;
        // Returning on the first error would leave running workers detached.
        let mut first_error = None;

        let progress_bar = logger.map(|logger| ScopedProgressBar::new(logger, self.tasks.len() as u64));

        // After a failure no new task is started; running ones are waited for.
        while (next_task < self.tasks.len() && first_error.is_none()) || !threads.is_empty() {
            while threads.len() < num_threads && next_task < self.tasks.len() && first_error.is_none() {
                let task = &self.tasks[next_task];
                let executable_file = self.programs[task.program.id].clone();
                let input_data = Arc::clone(&task.input);
                let time_limit = task.time_limit;
                let timer_path = timer_path.clone();

                threads.push((spawn(move || run_solution(&executable_file, input_data, time_limit, &timer_path)), next_task));
                next_task += 1;
            }

            let mut still_running = Vec::new();
            for (thread, idx) in threads {
                if !thread.is_finished() {
                    still_running.push((thread, idx));
                    continue;
                }

                let result = thread.join().unwrap_or_else(|_panic| {
                    Err(Error::TimerFailed {
                        details: format!("the worker thread for task {idx} panicked"),
                    })
                });

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
            threads = still_running;

            if !threads.is_empty() {
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        first_error.map_or(Ok(()), Err)
    }
}
