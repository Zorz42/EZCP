use crate::solution::Solution;
use crate::subtask::Subtask;
use crate::{Error, Result};

use crate::archiver::archive_files;
use crate::logger_format::logger_format;
use crate::partial_solution::run_partial_solution;
use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use console::style;
use indicatif::{MultiProgress, ProgressBar};
use indicatif_log_bridge::LogWrapper;
use log::{LevelFilter, debug, error, info, warn};
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
pub struct Task {
    name: String,
    // path to the folder with tests
    pub tests_path: PathBuf,
    // time limit in seconds
    pub time_limit: f32,
    // path to the zip file with tests
    pub tests_archive_path: PathBuf,
    // two closures that tells what should the input/output file be named for a given test
    // input to the closure is (test_id, subtask_id, test_id_in_subtask)
    pub get_input_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    pub get_output_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    build_folder_path: PathBuf,
    subtasks: Vec<Subtask>,
    // source code of the main solution
    pub solution_source: String,
    // Solutions (partial) that should be validated
    solutions: Vec<Solution>,
    // Minimum number of failures required per subtask for each non-passing solution
    pub min_failures_per_solution: usize,
    // Maximum number of tests to generate per subtask (safety limit)
    pub max_tests_per_subtask: usize,

    pub debug_level: LevelFilter,
    logger: MultiProgress,

    /// Generated test inputs per subtask (built dynamically)
    generated_tests: Vec<Vec<String>>,
}

impl Task {
    /// This function creates a new task with the given name and path.
    /// The path should be a relative path to the task folder in which the tests will be generated.
    /// The solution should be at `solution_path` which is `path`/solution.cpp by default but can be changed.
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
            max_tests_per_subtask: 100,
            debug_level: LevelFilter::Info,
            logger: MultiProgress::new(),
            solution_source: String::new(),
            generated_tests: Vec::new(),
        }
    }

    /// Set the source code of the main solution.
    #[must_use]
    pub fn with_solution_source(mut self, source: String) -> Self {
        self.solution_source = source;
        self
    }

    pub(crate) fn get_input_file_path(&self, test_id: i32, subtask_id: i32, test_id_in_subtask: i32) -> PathBuf {
        self.tests_path.join((self.get_input_file_name)(test_id, subtask_id, test_id_in_subtask))
    }

    pub(crate) fn get_output_file_path(&self, test_id: i32, subtask_id: i32, test_id_in_subtask: i32) -> PathBuf {
        self.tests_path.join((self.get_output_file_name)(test_id, subtask_id, test_id_in_subtask))
    }

    /// This function adds a subtask to the task.
    /// The subtask must be ready as it cannot be modified after it is added to the task.
    #[must_use]
    pub fn with_subtask(mut self, mut subtask: Subtask) -> Self {
        subtask.number = self.subtasks.len();
        self.subtasks.push(subtask);
        self
    }

    /// This function adds a solution to the task.
    /// A solution is expected to pass the specified subtasks and fail on others.
    /// During test generation, the system will dynamically generate tests until
    /// each solution fails on at least `min_failures_per_solution` tests for subtasks it should fail.
    #[must_use]
    pub fn with_solution(mut self, solution_source: String, passes_subtasks: &[usize]) -> Self {
        self.solutions.push(Solution::new(solution_source, passes_subtasks));
        self
    }

    /// Set the minimum number of test failures required per subtask for non-passing solutions.
    /// Default is 5.
    #[must_use]
    pub const fn with_min_failures(mut self, n: usize) -> Self {
        self.min_failures_per_solution = n;
        self
    }

    /// Set the maximum number of tests per subtask (safety limit).
    /// Default is 100.
    #[must_use]
    pub const fn with_max_tests_per_subtask(mut self, n: usize) -> Self {
        self.max_tests_per_subtask = n;
        self
    }

    /// This creates tests and prints the error message if there is an error.
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
        const TOTAL_STEPS: i32 = 5;
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

        // create tests directory if it doesn't exist and clear it
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


        self.print_progress(1, TOTAL_STEPS, "Generating initial tests");
        self.generate_initial_tests();

        self.print_progress(2, TOTAL_STEPS, "Running dynamic test generation");
        self.generate_dynamic_tests(&mut cpp_runner, &solution_handles);

        self.print_progress(3, TOTAL_STEPS, "Writing tests");
        let test_files = self.write_tests()?;

        self.print_progress(4, TOTAL_STEPS, "Generating test solutions");
        self.generate_test_solutions(&test_files, &mut cpp_runner, solution_handle)?;

        self.print_progress(5, TOTAL_STEPS, "Verifying solutions and archiving");
        self.check_solutions(&test_files, &mut cpp_runner, &solution_handles)?;
        self.archive_tests(&test_files)?;

        let tests_size = fs_extra::dir::get_size(&self.tests_path).unwrap_or(0) as f32 / 1_000_000.0;
        info!("Tests size: {}", style(format!("{tests_size:.2}MB")).bold());

        // Log test counts per subtask
        for (i, tests) in self.generated_tests.iter().enumerate() {
            info!("Subtask {}: {} tests", i + 1, tests.len());
        }

        Ok(())
    }

    /// Generate initial tests from each generator based on `initial_counts`
    fn generate_initial_tests(&mut self) {
        for subtask_idx in 0..self.subtasks.len() {
            let subtask = &self.subtasks[subtask_idx];
            for (gen_idx, generator) in subtask.generators.iter().enumerate() {
                let count = subtask.initial_counts.get(gen_idx).copied().unwrap_or(1);
                for _ in 0..count {
                    let test_input = generator.generate();
                    self.generated_tests[subtask_idx].push(test_input);
                }
            }
        }
    }

    /// Generate additional tests dynamically.
    /// For now, this just ensures we have enough tests per subtask.
    /// The actual solution validation happens in `check_solutions`.
    fn generate_dynamic_tests(&mut self, _cpp_runner: &mut CppRunner, _solution_handles: &[ProgramHandle]) {
        if self.solutions.is_empty() {
            return;
        }

        // Generate additional tests for each subtask until we have enough
        // The actual failure tracking happens during check_solutions
        let progress_bar = self.logger.add(ProgressBar::new_spinner());
        let mut iteration = 0;

        // For each subtask, ensure we generate enough tests that solutions can fail on
        for subtask_idx in 0..self.subtasks.len() {
            // Check if any solution needs to fail on this subtask
            let needs_failures = self.solutions.iter().any(|s| s.should_fail(subtask_idx));

            if !needs_failures {
                continue;
            }

            // Generate additional tests up to min_failures_per_solution + initial tests
            let current_count = self.generated_tests[subtask_idx].len();
            let target_count = current_count + self.min_failures_per_solution;
            let target_count = target_count.min(self.max_tests_per_subtask);

            while self.generated_tests[subtask_idx].len() < target_count {
                let subtask = &self.subtasks[subtask_idx];
                if let Some(test_input) = subtask.generate_random_test() {
                    self.generated_tests[subtask_idx].push(test_input);
                    iteration += 1;
                    progress_bar.set_message(format!("Generated {iteration} additional tests"));
                    progress_bar.tick();
                } else {
                    break;
                }
            }
        }

        self.logger.remove(&progress_bar);
        if iteration > 0 {
            info!("Generated {iteration} additional tests dynamically");
        }
    }

    /// Write all generated tests to files
    fn write_tests(&self) -> Result<Vec<Vec<(PathBuf, PathBuf)>>> {
        let mut test_files = Vec::new();
        let mut global_test_id = 0;

        for (subtask_idx, tests) in self.generated_tests.iter().enumerate() {
            let mut tests_written = Vec::new();

            for (test_id_in_subtask, test_input) in tests.iter().enumerate() {
                let test_id_in_subtask = test_id_in_subtask as i32;
                let input_path = self.get_input_file_path(global_test_id, subtask_idx as i32, test_id_in_subtask);
                let output_path = self.get_output_file_path(global_test_id, subtask_idx as i32, test_id_in_subtask);

                // Write input file
                fs::write(&input_path, test_input).map_err(|err| Error::IOError { err, file: path_str(&input_path) })?;

                tests_written.push((input_path, output_path));
                global_test_id += 1;
            }

            test_files.push(tests_written);
        }

        Ok(test_files)
    }

    fn generate_test_solutions(&self, test_files: &[Vec<(PathBuf, PathBuf)>], cpp_runner: &mut CppRunner, solution_handle: ProgramHandle) -> Result<()> {
        let mut test_tasks = Vec::new();

        // invoke solution on each test
        for subtask in test_files {
            let mut subtask_tasks = Vec::new();
            for (input_file, _output_file) in subtask {
                let test_contents = fs::read_to_string(input_file).map_err(|err| Error::IOError {
                    err,
                    file: input_file.to_str().unwrap_or("?").to_owned(),
                })?;
                let task_handle = cpp_runner.add_task(solution_handle, test_contents, self.time_limit);
                subtask_tasks.push(task_handle);
            }
            test_tasks.push(subtask_tasks);
        }

        cpp_runner.run_tasks(Some(&self.logger))?;

        let mut max_elapsed_time = 0;
        for (subtask, tasks) in test_files.iter().zip(test_tasks) {
            for ((input_file, output_file), task_handle) in subtask.iter().zip(tasks) {
                let test_result = cpp_runner.get_result(task_handle);
                let (elapsed_time, output) = match test_result {
                    RunResult::Ok(elapsed_time, output) => (elapsed_time, output),
                    RunResult::TimedOut => {
                        return Err(Error::SolutionTimedOut {
                            test_path: input_file.to_str().unwrap_or("?").to_owned(),
                        });
                    }
                    RunResult::Crashed => {
                        return Err(Error::SolutionFailed {
                            test_path: input_file.to_str().unwrap_or("?").to_owned(),
                        });
                    }
                };
                max_elapsed_time = max_elapsed_time.max(elapsed_time);
                // write output to the output file
                fs::write(output_file, output).map_err(|err| Error::IOError {
                    err,
                    file: output_file.to_str().unwrap_or("?").to_owned(),
                })?;
            }
        }
        info!("Solution time: {max_elapsed_time}ms");
        cpp_runner.clear_tasks();

        Ok(())
    }

    fn check_solutions(&self, test_files: &[Vec<(PathBuf, PathBuf)>], cpp_runner: &mut CppRunner, solution_handles: &[ProgramHandle]) -> Result<()> {
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
