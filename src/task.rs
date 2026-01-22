use crate::solution::Solution;
use crate::subtask::Subtask;
use crate::{Error, Result};

use crate::archiver::archive_files;
use crate::logger_format::logger_format;
//use crate::partial_solution::run_partial_solution;
use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use console::style;
use indicatif::{MultiProgress, ProgressBar};
use indicatif_log_bridge::LogWrapper;
use log::{LevelFilter, debug, error, info, warn};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::collections::HashSet;

pub static LOGGER_INIT: Once = Once::new();

// Convert a Path to an owned String for error contexts and logs
fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// This struct represents an entire task.
/// You can add subtasks, solutions (main and partial) and set the time limit.
/// Once you are done, you can create tests for the task.
///
/// The system dynamically generates tests until each solution that is expected
/// to fail on a subtask has failed on at least `min_failures_per_solution` tests.
/// Represents an entire competitive programming task.
///
/// A `Task` manages subtasks, solutions, and test generation configurations.
/// It uses a builder-like pattern to set up the problem before running the
/// test generation and verification process.
pub struct Task {
    /// Name of the task
    name: String,
    /// Directory where generated tests will be saved
    pub tests_path: PathBuf,
    /// Time limit in seconds for solutions
    pub time_limit: f32,
    /// Path to the final ZIP archive containing all tests
    pub tests_archive_path: PathBuf,
    /// Closure to determine input file names: `(test_id, subtask_id, id_in_subtask) -> String`
    pub get_input_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    /// Closure to determine output file names: `(test_id, subtask_id, id_in_subtask) -> String`
    pub get_output_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    /// Internal build directory for compiling solutions
    build_folder_path: PathBuf,
    /// Registered subtasks
    subtasks: Vec<Subtask>,
    /// Source code of the correct (main) solution
    pub solution_source: String,
    /// Partial solutions to be verified against subtasks
    solutions: Vec<Solution>,
    /// Target number of failures per "bad" solution per subtask
    pub min_failures_per_solution: usize,
    /// Maximum number of tests allowed per subtask
    pub max_tests_per_subtask: usize,

    /// Log level for output
    pub debug_level: LevelFilter,
    /// Progress reporting manager
    logger: MultiProgress,

    /// Storage for generated test inputs: `[subtask_idx][test_idx]`
    generated_tests: Vec<Vec<String>>,
}

impl Task {
    /// Creates a new `Task` with the given name and root directory.
    ///
    /// * `name` - Descriptive name for the task.
    /// * `path` - Root directory where tests and build files will be stored.
    #[must_use]
    pub fn new(name: &str, path: &Path) -> Self {
        let build_folder_path = path.join("build");
        Self {
            name: name.to_owned(),
            tests_path: path.join("tests"),
            tests_archive_path: path.join("tests.zip"),
            get_input_file_name: Box::new(|test_id, subtask_id, _test_id_in_subtask| {
                format!("test.{:02}.{:03}.in", subtask_id + 1, test_id + 1)
            }),
            get_output_file_name: Box::new(|test_id, subtask_id, _test_id_in_subtask| {
                format!("test.{:02}.{:03}.out", subtask_id + 1, test_id + 1)
            }),
            build_folder_path,
            time_limit: 5.0,
            subtasks: Vec::new(),
            solutions: Vec::new(),
            min_failures_per_solution: 5,
            max_tests_per_subtask: 100,
            debug_level: LevelFilter::Info,
            logger: MultiProgress::new(),
            solution_source: String::new(),
            generated_tests: Vec::new(),
        }
    }

    /// Sets the source code of the correct (main) solution.
    #[must_use]
    pub fn with_solution_source(mut self, source: String) -> Self {
        self.solution_source = source;
        self
    }

    /// Internal helper to get input file path.
    pub(crate) fn get_input_file_path(&self, test_id: i32, subtask_id: i32, test_id_in_subtask: i32) -> PathBuf {
        self.tests_path.join((self.get_input_file_name)(test_id, subtask_id, test_id_in_subtask))
    }

    /// Internal helper to get output file path.
    pub(crate) fn get_output_file_path(&self, test_id: i32, subtask_id: i32, test_id_in_subtask: i32) -> PathBuf {
        self.tests_path.join((self.get_output_file_name)(test_id, subtask_id, test_id_in_subtask))
    }

    /// Adds a subtask to the task.
    #[must_use]
    pub fn with_subtask(mut self, mut subtask: Subtask) -> Self {
        subtask.number = self.subtasks.len();
        self.subtasks.push(subtask);
        self
    }

    /// Adds a solution (partial or incorrect) to be verified.
    ///
    /// * `passes_subtasks` - List of subtask indices this solution is expected to pass.
    #[must_use]
    pub fn with_solution(mut self, solution_source: String, passes_subtasks: &[usize]) -> Self {
        self.solutions.push(Solution::new(solution_source, passes_subtasks));
        self
    }

    /// Sets the minimum number of failures required per subtask for incorrect solutions.
    #[must_use]
    pub const fn with_min_failures(mut self, n: usize) -> Self {
        self.min_failures_per_solution = n;
        self
    }

    /// Sets a safety limit on the number of tests per subtask.
    #[must_use]
    pub const fn with_max_tests_per_subtask(mut self, n: usize) -> Self {
        self.max_tests_per_subtask = n;
        self
    }

    /// Executes the task: compiles solutions, generates robust tests, and verifies outcomes.
    pub fn run(mut self) -> Result<()> {
        LOGGER_INIT.call_once(|| {
            let mut builder = env_logger::builder();
            builder.filter(None, self.debug_level);
            builder.format(logger_format);
            let env_logger_instance = builder.build();

            LogWrapper::new(self.logger.clone(), env_logger_instance).try_init().ok();
            log::set_max_level(self.debug_level);
            debug!("Logger initialized with level: {}", self.debug_level);
        });

        let start_time = std::time::Instant::now();
        let res = self.create_tests_inner();
        if let Err(err) = res {
            error!("{}", style(&err).red().bright());
            Err(err)
        } else {
            info!("Elapsed time: {}", style(format!("{:.2}s", start_time.elapsed().as_secs_f32())).bold());
            self.logger.println(format!("{}", style("Success!").green().bright().bold())).ok();
            Ok(())
        }
    }

    fn print_progress(&self, curr: i32, total: i32, text: &str) {
        self.logger.println(format!("[{}/{}] {}", style(curr).bold(), style(total).bold(), style(text).cyan().bold())).ok();
    }

    fn print_title(&self, text: &str) {
        // print title with ===== before and after text
        let mut border_text = String::from(" ");
        for _ in 0..text.len() + 6 {
            border_text.push('=');
        }
        self.logger.println(&border_text).ok();
        self.logger.println(format!(" || {} ||", style(text).bold())).ok();
        self.logger.println(&border_text).ok();
    }

    /// This function builds solution and then calls `generate_tests`.
    fn create_tests_inner(&mut self) -> Result<()> {
        self.logger.println("").ok();
        let text = format!("Creating tests for task \"{}\"", self.name);
        self.print_title(&text);

        if self.subtasks.is_empty() {
            warn!("No subtasks defined.");
        }

        // create build directory if it doesn't exist
        if !self.build_folder_path.exists() {
            fs::create_dir_all(&self.build_folder_path).map_err(|err| Error::IOError {
                err,
                file: path_str(&self.build_folder_path),
            })?;
        }

        // check if solution source exists
        if self.solution_source.is_empty() {
            return Err(Error::MissingSolution {});
        }

        // Initialize generated_tests storage
        self.generated_tests = vec![Vec::new(); self.subtasks.len()];

        let mut cpp_runner = CppRunner::new(&self.build_folder_path)?;
        let solution_handle = cpp_runner.add_program(&self.solution_source)?;
        let mut solution_handles = Vec::new();
        for solution in &self.solutions {
            solution_handles.push(cpp_runner.add_program(&solution.source)?);
        }

        // Prepare test directory
        if self.tests_path.exists() {
            fs::remove_dir_all(&self.tests_path).map_err(|err| Error::IOError { err, file: path_str(&self.tests_path) })?;
        }
        fs::create_dir_all(&self.tests_path).map_err(|err| Error::IOError { err, file: path_str(&self.tests_path) })?;

        let num_subtasks = self.subtasks.len();
        let mut global_test_id = 0;
        let mut all_test_files = Vec::new();

        for (subtask_idx, subtask) in self.subtasks.iter().enumerate() {
            const MAX_TRIES: usize = 500;
            self.print_progress((subtask_idx + 1) as i32, num_subtasks as i32, &format!("Subtask {}", subtask_idx + 1));
            let bad_solution_handles: Vec<ProgramHandle> = self.solutions.iter().zip(&solution_handles)
                 .filter(|(sol, _)| !sol.passes_subtasks.contains(&subtask_idx))
                 .map(|(_, &handle)| handle)
                 .collect();
            
            let target_count = if bad_solution_handles.is_empty() {
                 subtask.initial_test_count().max(1)
            } else {
                 self.min_failures_per_solution
            };
            
            let mut tried_inputs = HashSet::new();
            let mut found_count = 0;
            let mut tries = 0;
            let subtask_start_time = std::time::Instant::now();
            let subtask_timeout = std::time::Duration::from_secs(30);

            let progress_bar = self.logger.add(ProgressBar::new(target_count as u64));
            
            while found_count < target_count && tries < MAX_TRIES && subtask_start_time.elapsed() < subtask_timeout {
                 tries += 1;
                 let Some(candidate) = subtask.generate_random_test() else { break };
                 if tried_inputs.contains(&candidate) { continue; }
                 tried_inputs.insert(candidate.clone());
                 
                 if self.is_robust_test(&candidate, solution_handle, &bad_solution_handles, &mut cpp_runner)? {
                     self.generated_tests[subtask_idx].push(candidate);
                     found_count += 1;
                     progress_bar.inc(1);
                 }
            }
            if found_count < target_count {
                 warn!("Could not find enough robust tests for Subtask {} (found {}/{})", subtask_idx+1, found_count, target_count);
            }
            self.logger.remove(&progress_bar);

            // Write and solve subtask tests immediately
            let mut subtask_files = Vec::new();
            for (test_id_in_subtask, test_input) in self.generated_tests[subtask_idx].iter().enumerate() {
                let input_path = self.get_input_file_path(global_test_id, subtask_idx as i32, test_id_in_subtask as i32);
                let output_path = self.get_output_file_path(global_test_id, subtask_idx as i32, test_id_in_subtask as i32);
                fs::write(&input_path, test_input).map_err(|err| Error::IOError { err, file: path_str(&input_path) })?;
                
                let test_result = cpp_runner.check_programs(test_input, &[solution_handle], self.time_limit)?;
                let output = match &test_result[0] {
                    RunResult::Ok(_, output) => output,
                    RunResult::TimedOut => return Err(Error::SolutionTimedOut { test_path: path_str(&input_path) }),
                    RunResult::Crashed => return Err(Error::SolutionFailed { test_path: path_str(&input_path) }),
                };
                fs::write(&output_path, output).map_err(|err| Error::IOError { err, file: path_str(&output_path) })?;
                
                subtask_files.push((input_path, output_path));
                global_test_id += 1;
            }
            all_test_files.push(subtask_files);
        }

        // Final verification of all partial solutions
        //self.check_solutions(&all_test_files, &mut cpp_runner, &solution_handles)?;
        self.archive_tests(&all_test_files)?;

        let tests_size = fs_extra::dir::get_size(&self.tests_path).unwrap_or(0) as f32 / 1_000_000.0;
        info!("Tests size: {}", style(format!("{tests_size:.2}MB")).bold());

        // Log test counts per subtask
        for (i, tests) in self.generated_tests.iter().enumerate() {
            info!("Subtask {}: {} tests", i + 1, tests.len());
        }

        Ok(())
    }

    /// Checks if a candidate test input effectively distinguishes between the correct solution
    /// and a set of "bad" solutions.
    ///
    /// A test is considered robust if:
    /// 1. The main solution produces a valid result (doesn't TLE or crash).
    /// 2. Every "bad" solution either TLEs, crashes, or produces a different output
    ///    than the correct solution.
    fn is_robust_test(&self, input: &str, main_prog: ProgramHandle, bad_progs: &[ProgramHandle], runner: &mut CppRunner) -> Result<bool> {
        // Run Correct (Main) Solution
        let main_result = runner.check_programs(input, &[main_prog], self.time_limit)?;
        
        let main_output = match &main_result[0] {
             RunResult::Ok(_, output) => output.trim().to_owned(),
             RunResult::TimedOut => return Err(Error::SolutionTimedOut {
                 test_path: "generation phase".to_owned(),
             }),
             RunResult::Crashed => return Err(Error::SolutionFailed {
                 test_path: "generation phase".to_owned(),
             }),
        };
        
        if bad_progs.is_empty() {
            return Ok(true);
        }

        // Run Bad Solutions to ensure they fail
        let bad_results = runner.check_programs(input, bad_progs, self.time_limit)?;
        
        for res in bad_results {
            match res {
                RunResult::Ok(_, output) if output.trim() == main_output => {
                    // A bad solution passed this test! This test is not robust enough.
                    return Ok(false);
                }
                _ => {} // Bad solution failed as expected (TLE, Crash, or WA)
            }
        }
        Ok(true)
    }



    // /// Verifies all registered solutions against the generated test suite.
    // ///
    // /// Throws an error if any solution fails a subtask it was expected to pass,
    // /// or if it passes a subtask it was expected to fail.
    /*fn check_solutions(&self, test_files: &[Vec<(PathBuf, PathBuf)>], cpp_runner: &mut CppRunner, solution_handles: &[ProgramHandle]) -> Result<()> {
        for (sol_idx, solution) in self.solutions.iter().enumerate() {
            info!("Running solution {}:", sol_idx + 1);

            let subtasks_passed = run_partial_solution(test_files, cpp_runner, solution_handles[sol_idx], &self.logger, self.time_limit)?;

            for subtask_id in 0..self.subtasks.len() {
                let subtask_failed = !subtasks_passed.contains(&subtask_id);

                if subtask_failed && solution.passes_subtasks.contains(&subtask_id) {
                    return Err(Error::PartialSolutionFailsSubtask {
                        partial_number: sol_idx + 1,
                        subtask_number: subtask_id + 1,
                    });
                }

                if !subtask_failed && !solution.passes_subtasks.contains(&subtask_id) {
                    return Err(Error::PartialSolutionPassesExtraSubtask {
                        partial_number: sol_idx + 1,
                        subtask_number: subtask_id + 1,
                    });
                }
            }
        }

        Ok(())
    }*/

    /// Archive all tests into a zip file
    fn archive_tests(&self, test_files: &[Vec<(PathBuf, PathBuf)>]) -> Result<()> {
        let mut test_files_vec = Vec::new();
        for subtask in test_files {
            for (input_file, output_file) in subtask {
                test_files_vec.push(input_file.clone());
                test_files_vec.push(output_file.clone());
            }
        }

        archive_files(&test_files_vec, &self.tests_archive_path, &self.logger)?;

        Ok(())
    }
}
