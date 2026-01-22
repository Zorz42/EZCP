use crate::Error::IOError;
use crate::Result;
use crate::runner::exec_runner::{RunResult, run_solution};
use crate::runner::gcc::{Gcc, GccOptimization, GccStandard};
use indicatif::{MultiProgress, ProgressBar};
use log::trace;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::thread::spawn;
use std::time::Duration;

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// A unique handle for a compiled C++ program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProgramHandle {
    pub(crate) id: usize,
}

struct Task {
    program: ProgramHandle,
    input: String,
    time_limit: f32, // in seconds
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
        let build_folder = build_folder.canonicalize().map_err(|err| IOError {
            err,
            file: build_folder.to_string_lossy().to_string(),
        })?;
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

        self.hash_to_handle.insert(hash, handle);
        let source_file = self.build_folder.join(format!("p{hash}.cpp"));
        let executable_file = Gcc::transform_output_file(&source_file, None)?;
        
        self.necessary_files.insert(source_file.clone());
        self.necessary_files.insert(executable_file.clone());

        if !source_file.exists() {
            std::fs::write(&source_file, source_code).map_err(|err| IOError {
                err,
                file: path_str(&source_file),
            })?;
        }

        if !executable_file.exists() {
            trace!("Compiling: {}", executable_file.to_string_lossy());
            self.gcc.compile(&source_file, Some(&executable_file))?;
        }

        self.programs.push(executable_file);
        Ok(handle)
    }

    /// Registers a new execution task.
    ///
    /// * `program` - Handle to the executable to run.
    /// * `input` - Data to be sent to stdin.
    /// * `time_limit` - Maximum CPU time in seconds.
    pub fn add_task(&mut self, program: ProgramHandle, input: String, time_limit: f32) -> TaskHandle {
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

    /// Retrieves the result of a completed task.
    ///
    /// # Panics
    /// Panics if the task has not finished running.
    pub fn get_result(&self, task_handle: TaskHandle) -> RunResult {
        self.tasks[task_handle.id].result.clone().expect("Task result not available")
    }

    /// Runs multiple programs against a single input sequentially or in parallel.
    ///
    /// This is a convenience method that manages task creation and result collection.
    pub fn check_programs(&mut self, input: &str, programs: &[ProgramHandle], time_limit: f32) -> Result<Vec<RunResult>> {
        self.clear_tasks();
        let mut handles = Vec::new();
        for &program in programs {
            handles.push(self.add_task(program, input.to_owned(), time_limit));
        }
        self.run_tasks_internal(None, false)?;
        let mut results = Vec::new();
        for handle in handles {
            results.push(self.get_result(handle));
        }
        self.clear_tasks();
        Ok(results)
    }

    /// Deletes all files in the build directory that are not associated with
    /// currently registered programs.
    fn clean_build_folder(&self) -> Result<()> {
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
            if !self.necessary_files.contains(&path) {
                std::fs::remove_file(&path).map_err(|err| IOError {
                    err,
                    file: path_str(&path),
                })?;
            }
        }
        Ok(())
    }

    /// Executes all registered tasks in parallel.
    ///
    /// * `logger` - Optional MultiProgress for visually reporting progress.
    pub fn run_tasks(&mut self, logger: Option<&MultiProgress>) -> Result<()> {
        self.run_tasks_internal(logger, true)
    }

    fn run_tasks_internal(&mut self, logger: Option<&MultiProgress>, clean: bool) -> Result<()> {
        if clean {
            self.clean_build_folder()?;
        }

        let timer_path = self.programs[self.timer.id].clone();

        let num_threads = num_cpus::get().min(4);
        let mut threads = Vec::new();

        let mut it = 0;

        let progress_bar = logger.map(|logger| logger.add(ProgressBar::new(self.tasks.len() as u64)));

        loop {
            while threads.len() < num_threads && it < self.tasks.len() {
                let program_handle = &self.tasks[it].program;
                let executable_file = self.programs[program_handle.id].clone();
                let input_data = self.tasks[it].input.clone();
                let time_limit = self.tasks[it].time_limit;

                it += 1;
                if let Some(progress_bar) = &progress_bar {
                    progress_bar.inc(1);
                }

                let timer_path = timer_path.clone();
                threads.push((spawn(move || run_solution(&executable_file, &input_data, time_limit, &timer_path)), it - 1));
            }

            let mut threads_upd = Vec::new();
            for (thread, idx) in threads {
                if thread.is_finished() {
                    let result = thread.join().unwrap()?;
                    trace!("Task {idx} finished with result: {result:?}");
                    self.tasks[idx].result = Some(result);
                } else {
                    threads_upd.push((thread, idx));
                }
            }

            threads = threads_upd;

            std::thread::sleep(Duration::from_millis(1));

            if it == self.tasks.len() && threads.is_empty() {
                break;
            }
        }

        if let Some(logger) = logger {
            logger.remove(&progress_bar.unwrap());
        }

        Ok(())
    }
}
