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
use std::thread::{JoinHandle, spawn};
use std::time::Duration;

/// How many solutions may run at the same time.
///
/// Verdicts depend on how much CPU a solution gets to use, so the point of the
/// cap is to keep the machine from being oversubscribed by the runs themselves:
/// past a handful of solutions the extra parallelism mostly buys contention.
const MAX_CONCURRENT_SOLUTIONS: usize = 4;

/// A unique handle for a compiled C++ program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProgramHandle {
    pub(crate) id: usize,
}

struct Task {
    program: ProgramHandle,
    /// Shared, so handing a task to a worker thread does not copy the whole test.
    input: Arc<str>,
    time_limit: i32, // in milliseconds
    result: Option<RunResult>,
}

/// A unique handle for an asynchronous execution task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskHandle {
    pub(crate) id: usize,
}

/// Orchestrates the compilation and parallel execution of C++ solutions.
///
/// `CppRunner` manages a build folder, handles program deduplication via hashing,
/// and provides an asynchronous task-based API for running binaries with time limits.
pub struct CppRunner {
    /// Interface to the system's C++ compiler
    gcc: Gcc,
    /// Directory where source files and binaries are stored
    build_folder: PathBuf,
    /// Handle to the internal timer utility
    timer: ProgramHandle,
    /// Map from program ID to executable path
    programs: Vec<PathBuf>,
    /// List of registered execution tasks
    tasks: Vec<Task>,
    /// Map from source code hash to program handle for deduplication
    hash_to_handle: HashMap<u64, ProgramHandle>,
    /// Files that should be preserved in the build folder
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
            timer: ProgramHandle { id: 0 }, // Timer will be built later
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

    /// Compiles a C++ source string and returns a handle to the executable.
    ///
    /// If the same source has already been added, the existing handle is returned.
    pub fn add_program(&mut self, source_code: &str) -> Result<ProgramHandle> {
        trace!("Adding program with source code: {source_code}");
        let handle = ProgramHandle { id: self.programs.len() };
        let hash = {
            let mut s = DefaultHasher::new();
            source_code.hash(&mut s);
            s.finish()
        };

        // Reuse existing program if hashes match
        if let Some(existing_handle) = self.hash_to_handle.get(&hash) {
            trace!("Program already exists with id: {}", existing_handle.id);
            return Ok(*existing_handle);
        }

        let source_file = self.build_folder.join(format!("p{hash}.cpp"));
        let executable_file = Gcc::transform_output_file(&source_file, None)?;

        self.necessary_files.insert(source_file.clone());
        self.necessary_files.insert(executable_file.clone());

        // Always rewrite the source. Skipping it when the file exists means a run
        // that was interrupted mid-write leaves a truncated source behind that is
        // never repaired, and every later run fails to compile it.
        std::fs::write(&source_file, source_code).map_err(|err| IOError { err, file: path_str(&source_file) })?;

        if !executable_file.exists() {
            trace!("Compiling: {}", executable_file.to_string_lossy());
            self.gcc.compile(&source_file, Some(&executable_file))?;
        }

        // Record the program only once it really exists. Remembering the handle up
        // front would make a later `add_program` of the same source hand out an id
        // that was never filled in, so it would silently address the next program
        // that did compile.
        self.programs.push(executable_file);
        self.hash_to_handle.insert(hash, handle);
        Ok(handle)
    }

    /// Registers a new execution task.
    ///
    /// * `program` - Handle to the executable to run.
    /// * `input` - Data to be sent to stdin.
    /// * `time_limit` - Maximum CPU time in milliseconds.
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

    /// Removes all registered tasks.
    pub fn clear_tasks(&mut self) {
        self.tasks.clear();
    }

    /// Moves the input of a task out of the runner.
    ///
    /// Once a task has run the runner has no further use for its input, while the
    /// caller usually needs it one more time to run the checker. Handing it over
    /// saves reading the whole test back from disk, and dropping the runner's
    /// reference releases the test as soon as it has been checked.
    pub fn take_input(&mut self, task_handle: TaskHandle) -> Arc<str> {
        std::mem::take(&mut self.tasks[task_handle.id].input)
    }

    /// Retrieves the result of a completed task.
    ///
    /// # Panics
    /// Panics if the task has not finished running.
    #[allow(clippy::expect_used)]
    pub fn get_result(&self, task_handle: TaskHandle) -> RunResult {
        self.tasks[task_handle.id].result.clone().expect("Task result not available")
    }

    /// Runs multiple programs against a single input sequentially or in parallel.
    ///
    /// This is a convenience method that manages task creation and result collection.
    pub fn check_programs(&mut self, input: &str, programs: &[ProgramHandle], time_limit: i32) -> Result<Vec<RunResult>> {
        self.clear_tasks();
        // One copy of the input for all of the programs, however many there are.
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

    /// Deletes all files in the build directory that are not associated with
    /// currently registered programs.
    ///
    /// Every edit to a solution compiles to a new binary under a new name, so
    /// without this the build folder keeps every binary the task has ever had.
    /// Call it once all programs have been added, never before: a binary that is
    /// removed here has to be compiled again.
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
            // Only ever remove plain files, so a directory somebody put in the
            // build folder is left alone.
            if !self.necessary_files.contains(&path) && path.is_file() {
                // Best effort: a leftover we cannot delete (a binary another EZCP
                // run still has open, an antivirus holding the file on Windows)
                // costs some disk space, which is no reason to fail the run.
                if let Err(err) = std::fs::remove_file(&path) {
                    warn!("Could not remove {} from the build folder: {err}", path_str(&path));
                }
            }
        }
        Ok(())
    }

    pub fn run_tasks(&mut self, logger: Option<&MultiProgress>) -> Result<()> {
        let timer_path = self.programs[self.timer.id].clone();

        let num_threads = num_cpus::get().min(MAX_CONCURRENT_SOLUTIONS);
        let mut threads: Vec<(JoinHandle<Result<RunResult>>, usize)> = Vec::new();

        let mut next_task = 0;
        // Hold on to the first failure rather than returning straight away: every
        // worker that is already running has a solution process attached to it, and
        // leaving through `?` would detach both and leave them running.
        let mut first_error = None;

        let progress_bar = logger.map(|logger| ScopedProgressBar::new(logger, self.tasks.len() as u64));

        // Once something has failed no further task is started, so the remaining
        // ones stop counting towards the work left to do; only the workers that
        // are already running still have to be waited for.
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
