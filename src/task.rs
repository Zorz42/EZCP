use crate::solution::Solution;
use crate::subtask::Subtask;
use crate::{Error, Result};

use crate::archiver::archive_files;
use crate::logger_format::logger_format;
use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use console::style;
use indicatif::{MultiProgress, ProgressBar};
use indicatif_log_bridge::LogWrapper;
use log::{LevelFilter, debug, error, info, warn};
use rand::seq::SliceRandom;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

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
    tests_path: PathBuf,
    /// Time limit in seconds for solutions
    time_limit: f32,
    /// Path to the final ZIP archive containing all tests
    tests_archive_path: PathBuf,
    /// Closure to determine input file names: `(test_id, subtask_id, id_in_subtask) -> String`
    get_input_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    /// Closure to determine output file names: `(test_id, subtask_id, id_in_subtask) -> String`
    get_output_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    /// Internal build directory for compiling solutions
    build_folder_path: PathBuf,
    /// Registered subtasks
    subtasks: Vec<Subtask>,
    /// Source code of the correct (main) solution
    solution_source: String,
    /// Partial solutions to be verified against subtasks
    solutions: Vec<Solution>,
    /// Target number of failures per "bad" solution per subtask
    min_failures_per_solution: usize,
    /// Maximum number of consecutive failed attempts to find a robust test
    max_tries: usize,

    /// Log level for output
    debug_level: LevelFilter,
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
            get_input_file_name: Box::new(|test_id, subtask_id, _test_id_in_subtask| format!("test.{:02}.{:03}.in", subtask_id + 1, test_id + 1)),
            get_output_file_name: Box::new(|test_id, subtask_id, _test_id_in_subtask| format!("test.{:02}.{:03}.out", subtask_id + 1, test_id + 1)),
            build_folder_path,
            time_limit: 5.0,
            subtasks: Vec::new(),
            solutions: Vec::new(),
            min_failures_per_solution: 5,
            max_tries: 100,
            debug_level: LevelFilter::Info,
            logger: MultiProgress::new(),
            solution_source: String::new(),
            generated_tests: Vec::new(),
        }
    }

    /// Sets the source code of the correct (main) solution.
    ///
    /// Panics if it is called the second time.
    #[must_use]
    pub fn with_solution_source(mut self, source: &str) -> Self {
        assert!(self.solution_source.is_empty());
        self.solution_source = source.to_owned();
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
    pub fn with_partial_solution(mut self, solution_source: &str, passes_subtasks: &[usize]) -> Self {
        self.solutions.push(Solution::new(solution_source.to_owned(), passes_subtasks));
        self
    }

    /// Sets the minimum number of failures required per subtask for incorrect solutions.
    #[must_use]
    pub const fn with_min_failures(mut self, n: usize) -> Self {
        self.min_failures_per_solution = n;
        self
    }

    /// Sets the maximum number of consecutive failed attempts to find a robust test.
    #[must_use]
    pub const fn with_max_tries(mut self, n: usize) -> Self {
        self.max_tries = n;
        self
    }

    /// Sets the directory for build artifacts.
    #[must_use]
    pub fn with_build_folder_path(mut self, path: PathBuf) -> Self {
        self.build_folder_path = path;
        self
    }

    /// Sets the directory where generated tests will be saved.
    #[must_use]
    pub fn with_tests_path(mut self, path: PathBuf) -> Self {
        self.tests_path = path;
        self
    }

    /// Sets the time limit in seconds for solutions.
    #[must_use]
    pub const fn with_time_limit(mut self, limit: f32) -> Self {
        self.time_limit = limit;
        self
    }

    /// Sets the path to the final ZIP archive containing all tests.
    #[must_use]
    pub fn with_tests_archive_path(mut self, path: PathBuf) -> Self {
        self.tests_archive_path = path;
        self
    }

    /// Sets the closure to determine input file names.
    #[must_use]
    pub fn with_get_input_file_name<F: Fn(i32, i32, i32) -> String + 'static>(mut self, f: F) -> Self {
        self.get_input_file_name = Box::new(f);
        self
    }

    /// Sets the closure to determine output file names.
    #[must_use]
    pub fn with_get_output_file_name<F: Fn(i32, i32, i32) -> String + 'static>(mut self, f: F) -> Self {
        self.get_output_file_name = Box::new(f);
        self
    }

    /// Sets the log level for output.
    #[must_use]
    pub const fn with_debug_level(mut self, level: LevelFilter) -> Self {
        self.debug_level = level;
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

        // add all cpp files (solution and partial solutions)
        let mut cpp_runner = CppRunner::new(&self.build_folder_path)?;
        let solution_handle = cpp_runner.add_program(&self.solution_source)?;
        let mut solution_handles = Vec::new();
        for solution in &self.solutions {
            solution_handles.push(cpp_runner.add_program(&solution.source)?);
        }

        // Prepare test directory
        if self.tests_path.exists() {
            fs::remove_dir_all(&self.tests_path).map_err(|err| Error::IOError {
                err,
                file: path_str(&self.tests_path),
            })?;
        }
        fs::create_dir_all(&self.tests_path).map_err(|err| Error::IOError {
            err,
            file: path_str(&self.tests_path),
        })?;

        let num_subtasks = self.subtasks.len();
        let mut global_test_id = 0;
        let mut all_test_files = Vec::new();

        for (subtask_idx, subtask) in self.subtasks.iter().enumerate() {
            self.print_progress((subtask_idx + 1) as i32, num_subtasks as i32, &format!("Subtask {}: {}", subtask_idx + 1, subtask.name));

            let mut good_solution_handles = Vec::new();
            let mut bad_solution_handles = Vec::new();
            for (i, solution) in self.solutions.iter().enumerate() {
                if solution.passes_subtasks.contains(&subtask_idx) {
                    good_solution_handles.push((i, solution_handles[i]));
                } else {
                    bad_solution_handles.push(solution_handles[i]);
                }
            }

            let mut tried_inputs = HashSet::new();
            let mut subtask_tests = Vec::new();
            let mut robust_found_count = 0;

            let total_initial: usize = subtask.initial_counts.iter().sum();
            let target_robust = if bad_solution_handles.is_empty() { 0 } else { self.min_failures_per_solution };

            let found_count_progress_bar = self.logger.add(ProgressBar::new((total_initial + target_robust) as u64));
            let tries_progress_bar = self.logger.add(ProgressBar::new(self.max_tries as u64));

            // Phase 1: Initial tests from each generator (only good solutions must pass)
            for (gen_idx, generator) in subtask.generators.iter().enumerate() {
                let needed = subtask.initial_counts[gen_idx];
                for _ in 0..needed {
                    let candidate = generator.generate();
                    // Each test must be unique within the subtask
                    if tried_inputs.contains(&candidate) {
                        continue;
                    }
                    tried_inputs.insert(candidate.clone());

                    // We check only good solutions in Phase 1 (no bad_progs passed)
                    let main_output = match self.is_robust_test(&candidate, solution_handle, &good_solution_handles, &[], &mut cpp_runner, subtask_idx)? {
                        Some(out) => out,
                        None => unreachable!("is_robust_test with no bad progs should always return Some or Err"),
                    };
                    subtask_tests.push((candidate, main_output));
                    found_count_progress_bar.inc(1);
                }
            }

            // Phase 2: Robust tests (failing bad solutions)
            let mut supplemental_tries = 0;
            while robust_found_count < target_robust && supplemental_tries < self.max_tries {
                supplemental_tries += 1;
                let Some(candidate) = subtask.generate_random_test() else { break };
                if tried_inputs.contains(&candidate) {
                    continue;
                }
                tried_inputs.insert(candidate.clone());

                if let Some(main_output) = self.is_robust_test(&candidate, solution_handle, &good_solution_handles, &bad_solution_handles, &mut cpp_runner, subtask_idx)? {
                    subtask_tests.push((candidate, main_output));
                    robust_found_count += 1;
                    supplemental_tries = 0;
                    found_count_progress_bar.inc(1);
                    tries_progress_bar.reset();
                }
                tries_progress_bar.inc(1);
            }

            if robust_found_count < target_robust {
                error!("Could not find enough robust tests for Subtask {} (found {}/{})", subtask_idx + 1, robust_found_count, target_robust);
            }
            self.logger.remove(&found_count_progress_bar);
            self.logger.remove(&tries_progress_bar);

            // Shuffle all tests for this subtask
            let mut rng = rand::rng();
            subtask_tests.shuffle(&mut rng);

            // Write shuffled tests to disk
            let mut subtask_files = Vec::new();
            for (test_id_in_subtask, (input, output)) in subtask_tests.into_iter().enumerate() {
                let input_path = self.get_input_file_path(global_test_id, subtask_idx as i32, test_id_in_subtask as i32);
                let output_path = self.get_output_file_path(global_test_id, subtask_idx as i32, test_id_in_subtask as i32);

                fs::write(&input_path, &input).map_err(|err| Error::IOError { err, file: path_str(&input_path) })?;
                fs::write(&output_path, output).map_err(|err| Error::IOError { err, file: path_str(&output_path) })?;

                subtask_files.push((input_path, output_path));
                self.generated_tests[subtask_idx].push(input);
                global_test_id += 1;
            }

            all_test_files.push(subtask_files);
        }

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
    /// 1. All "good" solutions (including main) produce the same valid response.
    /// 2. Every "bad" solution either TLEs, crashes, or produces a different output.
    fn is_robust_test(
        &self,
        input: &str,
        main_prog: ProgramHandle,
        good_progs: &[(usize, ProgramHandle)],
        bad_progs: &[ProgramHandle],
        runner: &mut CppRunner,
        subtask_idx: usize,
    ) -> Result<Option<String>> {
        let mut all_progs = vec![main_prog];
        for &(_, handle) in good_progs {
            all_progs.push(handle);
        }
        all_progs.extend_from_slice(bad_progs);

        // Run all solutions in parallel
        let results = runner.check_programs(input, &all_progs, self.time_limit)?;

        // Correct (Main) Solution Result
        let main_output = match &results[0] {
            RunResult::Ok(_, output) => output.trim().to_owned(),
            RunResult::TimedOut => {
                return Err(Error::SolutionTimedOut {
                    test_path: "generation phase".to_owned(),
                });
            }
            RunResult::Crashed => {
                return Err(Error::SolutionFailed {
                    test_path: "generation phase".to_owned(),
                });
            }
        };

        // Ensure all other "good" solutions pass and match main output
        for (i, &(sol_idx, _)) in good_progs.iter().enumerate() {
            match &results[1 + i] {
                RunResult::Ok(_, output) if output.trim() == main_output => {}
                _ => {
                    return Err(Error::PartialSolutionFailsSubtask {
                        partial_number: sol_idx + 1,
                        subtask_number: subtask_idx + 1,
                    });
                }
            }
        }

        if bad_progs.is_empty() {
            return Ok(Some(main_output));
        }

        // Run Bad Solutions to ensure they fail
        let bad_results_start = 1 + good_progs.len();
        for res in &results[bad_results_start..] {
            match res {
                RunResult::Ok(_, output) if output.trim() == main_output => {
                    // A bad solution passed this test! This test is not robust enough.
                    return Ok(None);
                }
                _ => {} // Bad solution failed as expected (TLE, Crash, or WA)
            }
        }
        Ok(Some(main_output))
    }

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
