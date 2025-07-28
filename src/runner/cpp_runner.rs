use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::thread::spawn;
use std::time::{Duration, SystemTime};
use indicatif::{MultiProgress, ProgressBar};
use log::trace;
use crate::runner::gcc::{Gcc, GccOptimization, GccStandard};
use crate::{Error, Result};
use crate::Error::IOError;
use crate::runner::runner::{run_solution, RunResult};

#[derive(Clone, Copy)]
pub struct ProgramHandle {
    id: usize,
}

struct Task {
    program: ProgramHandle,
    input: String,
    time_limit: f32, // in seconds
    result: Option<RunResult>,
}

#[derive(Clone, Copy)]
pub struct TaskHandle {
    id: usize,
}

fn get_file_modified_time(file: &PathBuf) -> Result<SystemTime> {
    let file_str1 = file.to_str().unwrap_or("???").to_owned();
    let file_str2 = file_str1.clone();
    std::fs::metadata(file)
        .map_err(|err| IOError { err, file: file_str1 })?
        .modified()
        .map_err(|err| IOError { err, file: file_str2 })
}

/// This struct is responsible for running C++ code.
/// You add multiple source codes into it as strings and receive a handle for each source code.
/// then you add tasks, which are triplets of (source code handle, input file, output file).
/// The tasks are run in parallel, and you can get the result of each task by its handle.
pub struct CppRunner {
    gcc: Gcc,
    build_folder: PathBuf,
    timer: ProgramHandle,
    // this stores the executable for each program
    programs: Vec<PathBuf>,
    // this stores the tasks to be run
    tasks: Vec<Task>,
    hash_to_handle: HashMap<u64, ProgramHandle>,
}

impl CppRunner {
    pub fn new(build_folder: PathBuf) -> Result<Self> {
        trace!("Creating CppRunner with build folder: {}", build_folder.to_string_lossy());
        if !build_folder.exists() {
            trace!("Build folder does not exist, creating: {}", build_folder.to_string_lossy());
            std::fs::create_dir_all(&build_folder)
                .map_err(|err| IOError { err, file: build_folder.to_string_lossy().to_string() })?;
        }
        let mut gcc = Gcc::new()?;
        gcc.standard = Some(GccStandard::Cpp17);
        gcc.optimization = Some(GccOptimization::Level2);
        let mut res = Self {
            gcc,
            build_folder,
            timer: ProgramHandle { id: 0 }, // Timer will be built later
            programs: Vec::new(),
            tasks: Vec::new(),
            hash_to_handle: HashMap::new(),
        };

        trace!("Building timer program");
        let timer_source = include_str!("timer.cpp");
        res.timer = res.add_program(timer_source)?;

        Ok(res)
    }

    pub fn add_program(&mut self, source_code: &str) -> Result<ProgramHandle> {
        trace!("Adding program with source code: {source_code}");
        let handle = ProgramHandle { id: self.programs.len() };
        let hash = {
            let mut s = DefaultHasher::new();
            source_code.hash(&mut s);
            s.finish()
        };

        // check if we already have this program
        if let Some(existing_handle) = self.hash_to_handle.get(&hash) {
            trace!("Program already exists with id: {}", existing_handle.id);
            return Ok(*existing_handle);
        }

        trace!("Program handle created with id: {} and hash: {hash}", handle.id);
        let source_file = self.build_folder.join(format!("p{}.cpp", hash));
        let executable_file = Gcc::transform_output_file(&source_file, None);

        if !source_file.exists() {
            trace!("Source file does not exist, writing to: {}", source_file.to_string_lossy());
            std::fs::write(&source_file, source_code)
                .map_err(|err| IOError { err, file: source_file.to_string_lossy().to_string() })?;
        }

        if !executable_file.exists() || get_file_modified_time(&executable_file)? < get_file_modified_time(&source_file)? {
            trace!("Executable file does not exist or is outdated, compiling: {}", executable_file.to_string_lossy());
            let compiled_executable = self.gcc.compile(&source_file, Some(&executable_file))?;

            // this should never happen, but just in case
            debug_assert_eq!(
                compiled_executable,
                executable_file,
                "GCC returned a different executable file than expected",
            );
        }

        self.programs.push(executable_file);
        Ok(handle)
    }

    pub fn add_task(&mut self, program: ProgramHandle, input: String, time_limit: f32) -> Result<TaskHandle> {
        trace!("Adding task for program id: {}, time limit: {}", program.id, time_limit);
        let handle = TaskHandle { id: self.tasks.len() };
        self.tasks.push(Task {
            program,
            input,
            time_limit,
            result: None,
        });
        Ok(handle)
    }

    pub fn clear_tasks(&mut self) {
        trace!("Clearing tasks");
        self.tasks.clear();
    }

    pub fn get_result(&mut self, task_handle: TaskHandle) -> RunResult {
        trace!("Getting result for task id: {}", task_handle.id);
        self.tasks[task_handle.id].result.clone().unwrap()
    }

    pub fn run_tasks(&mut self, logger: Option<&MultiProgress>) -> Result<()> {
        let timer_path = self.programs[self.timer.id].clone();

        let num_threads = num_cpus::get();
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