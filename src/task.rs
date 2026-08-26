use crate::solution::Solution;
use crate::subtask::Subtask;
use crate::{Error, Result};

use crate::archiver::archive_files;
use crate::logger_format::logger_format;
use crate::runner::cpp_runner::CppRunner;
use crate::to_output::ToOutput;
use console::style;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::{LevelFilter, debug, error, info, warn};
use std::collections::HashSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;

pub static LOGGER_INIT: Once = Once::new();

// Convert a Path to an owned String for error contexts and logs
pub fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Represents an entire competitive programming task.
///
/// A `Task` manages subtasks, solutions and test generation settings. It is put
/// together with a builder-like pattern and then run with [`Task::run`], which
/// compiles every solution, generates the tests and verifies the outcomes.
///
/// Test generation keeps going until each solution that is expected to fail on a
/// subtask has failed on at least `min_failures_per_solution` tests.
pub struct Task<T: ToOutput> {
    /// Name of the task
    pub(crate) name: String,
    /// Directory where the whole problem is stored
    pub(crate) problem_path: PathBuf,
    /// Directory where generated tests will be saved
    pub(crate) tests_path: PathBuf,
    /// Time limit in milliseconds for solutions
    pub(crate) time_limit: i32,
    /// Path to the final ZIP archive containing all tests
    pub(crate) tests_archive_path: PathBuf,
    /// Closure to determine input file names: `(test_id, subtask_id, id_in_subtask) -> String`
    pub(crate) get_input_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    /// Closure to determine output file names: `(test_id, subtask_id, id_in_subtask) -> String`
    pub(crate) get_output_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    /// Internal build directory for compiling solutions
    pub(crate) build_folder_path: PathBuf,
    /// Registered subtasks
    pub(crate) subtasks: Vec<Subtask<T>>,
    /// Source code of the correct (main) solution
    pub(crate) solution_source: String,
    /// Partial solutions to be verified against subtasks
    pub(crate) solutions: Vec<Solution>,
    /// Target number of failures per "bad" solution per subtask
    pub(crate) min_failures_per_solution: usize,
    /// Maximum number of consecutive failed attempts to find a robust test
    pub(crate) max_tries: usize,
    /// Test checker, used for problems with multiple different possible outputs.
    /// By default it is a diff checker (up to whitespace).
    /// The function takes 3 arguments: (`test_input`, `correct_output`, `program_output`)
    /// and returns `true` if the program output is accepted (correct), `false` if rejected.
    pub(crate) checker: fn(&str, &str, &str) -> bool,
    /// If you want to automatically trim whitespace from outputs
    pub(crate) trim_whitespace: bool,

    /// Log level for output
    pub(crate) debug_level: LevelFilter,
    /// Progress reporting manager
    pub(crate) logger: MultiProgress,
}

/// Removes a directory tree, retrying briefly before giving up.
///
/// The previous run leaves hundreds of test files behind, and on Windows an
/// antivirus scanner or the search indexer routinely still holds one of them
/// open for a moment, which makes a single `remove_dir_all` fail for no lasting
/// reason.
fn remove_dir_all_with_retry(path: &Path) -> Result<()> {
    const ATTEMPTS: u32 = 5;

    for attempt in 1..=ATTEMPTS {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(err) if attempt == ATTEMPTS => {
                return Err(Error::IOError { err, file: path_str(path) });
            }
            Err(err) => {
                debug!("Could not remove {} (attempt {attempt}/{ATTEMPTS}): {err}", path_str(path));
                std::thread::sleep(std::time::Duration::from_millis(50 * u64::from(attempt)));
            }
        }
    }

    Ok(())
}

fn diff_checker(_test_input: &str, official_output: &str, program_output: &str) -> bool {
    fn parse_whitespace(s: &str) -> Vec<&str> {
        let mut res = s.split_whitespace().collect::<Vec<_>>();
        res.retain(|x| !x.is_empty());
        res
    }
    parse_whitespace(official_output) == parse_whitespace(program_output)
}

impl<T: ToOutput> Task<T> {
    /// Creates a new `Task` with the given name and root directory.
    ///
    /// * `name` - Descriptive name for the task.
    /// * `path` - Root directory where tests and build files will be stored.
    #[must_use]
    pub fn new(name: &str, path: &Path) -> Self {
        let build_folder_path = path.join("build");
        Self {
            name: name.to_owned(),
            problem_path: path.to_owned(),
            tests_path: path.join("tests"),
            tests_archive_path: path.join("tests.zip"),
            get_input_file_name: Box::new(|test_id, subtask_id, _test_id_in_subtask| format!("test.{:02}.{:03}.in", subtask_id + 1, test_id + 1)),
            get_output_file_name: Box::new(|test_id, subtask_id, _test_id_in_subtask| format!("test.{:02}.{:03}.out", subtask_id + 1, test_id + 1)),
            build_folder_path,
            time_limit: 5000,
            subtasks: Vec::new(),
            solutions: Vec::new(),
            min_failures_per_solution: 5,
            max_tries: 100,
            debug_level: LevelFilter::Info,
            logger: MultiProgress::new(),
            solution_source: String::new(),
            checker: diff_checker,
            trim_whitespace: true,
        }
    }

    fn get_results_file(&self) -> PathBuf {
        self.problem_path.join("results.txt")
    }

    pub(crate) fn log_result(&self, text: &str) -> Result<()> {
        let results_file = self.get_results_file();
        let mut file = OpenOptions::new().append(true).create(true).open(&results_file).map_err(|e| Error::IOError {
            err: e,
            file: path_str(&results_file),
        })?;
        writeln!(file, "{}", console::strip_ansi_codes(text)).map_err(|e| Error::IOError {
            err: e,
            file: path_str(&results_file),
        })?;
        info!("{text}");
        Ok(())
    }

    /// Sets the source code of the correct (main) solution.
    ///
    /// # Panics
    /// Panics if it is called a second time.
    #[must_use]
    pub fn with_solution_source(mut self, source: &str) -> Self {
        assert!(self.solution_source.is_empty());
        self.solution_source = source.to_owned();
        self
    }

    /// Sets custom checker
    #[must_use]
    pub fn with_checker(mut self, checker: fn(&str, &str, &str) -> bool) -> Self {
        self.checker = checker;
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
    pub fn with_subtask(mut self, subtask: Subtask<T>) -> Self {
        self.subtasks.push(subtask);
        self
    }

    #[must_use]
    pub const fn trim_whitespace(mut self, trim_whitespace: bool) -> Self {
        self.trim_whitespace = trim_whitespace;
        self
    }

    /// Adds a solution (partial or incorrect) to be verified.
    ///
    /// * `passes_subtasks` - List of subtask indices this solution is expected to
    ///   pass, counted from zero. An index the task does not have is reported as
    ///   an error rather than ignored.
    ///
    /// Every other subtask has to reject the solution: test generation looks for
    /// tests that break it, and [`Task::run`] fails if the finished test data
    /// still lets it through a subtask it was not declared to pass.
    #[must_use]
    pub fn with_partial_solution(mut self, name: &str, source: &str, passes_subtasks: &[usize]) -> Self {
        self.solutions.push(Solution::new(name.to_owned(), source.to_owned(), passes_subtasks));
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

    /// Sets the time limit in milliseconds for solutions.
    ///
    /// The limit is on CPU time, so a machine under load does not turn a correct
    /// solution into a timeout.
    #[must_use]
    pub const fn with_time_limit(mut self, limit: i32) -> Self {
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
    ///
    /// The closure has to give every test its own name — it is passed the global
    /// test id, the subtask id and the id within the subtask for that. Two tests
    /// landing on the same name is reported as an error.
    #[must_use]
    pub fn with_get_input_file_name<F: Fn(i32, i32, i32) -> String + 'static>(mut self, f: F) -> Self {
        self.get_input_file_name = Box::new(f);
        self
    }

    /// Sets the closure to determine output file names.
    ///
    /// The same uniqueness requirement as for [`Task::with_get_input_file_name`]
    /// applies.
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
    pub fn run(self) -> Result<()> {
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
        // Measure how wide the title actually prints, not how many bytes it takes,
        // so a task name with non-ASCII characters still gets a matching border.
        let border_text = format!(" {}", "=".repeat(console::measure_text_width(text) + 6));
        self.logger.println(&border_text).ok();
        self.logger.println(format!(" || {} ||", style(text).bold())).ok();
        self.logger.println(&border_text).ok();
    }

    /// This function builds solution and then calls `generate_tests`.
    fn create_tests_inner(&self) -> Result<()> {
        self.logger.println("").ok();
        let text = format!("Creating tests for task \"{}\"", self.name);
        self.print_title(&text);

        if self.subtasks.is_empty() {
            warn!("No subtasks defined.");
        }

        self.check_declared_subtasks_exist()?;

        // create build directory if it doesn't exist
        if !self.build_folder_path.exists() {
            fs::create_dir_all(&self.build_folder_path).map_err(|err| Error::IOError {
                err,
                file: path_str(&self.build_folder_path),
            })?;
        }

        // check if solution source exists
        if self.solution_source.is_empty() {
            return Err(Error::MissingSolution);
        }
        // add all cpp files (solution and partial solutions)
        let mut cpp_runner = CppRunner::new(&self.build_folder_path)?;
        let solution_handle = cpp_runner.add_program(&self.solution_source)?;
        let mut solution_handles = Vec::new();
        for solution in &self.solutions {
            solution_handles.push(cpp_runner.add_program(&solution.source)?);
        }
        // Everything that will be run has been compiled by now, so whatever else
        // is still in the build folder is left over from an earlier run.
        cpp_runner.clean_build_folder()?;

        // Prepare test directory
        if self.tests_path.exists() {
            remove_dir_all_with_retry(&self.tests_path)?;
        }
        fs::create_dir_all(&self.tests_path).map_err(|err| Error::IOError {
            err,
            file: path_str(&self.tests_path),
        })?;

        // clear log file
        fs::File::create(self.get_results_file()).map_err(|e| Error::IOError {
            err: e,
            file: path_str(&self.get_results_file()),
        })?;

        let num_subtasks = self.subtasks.len();
        let mut global_test_id = 0;
        let mut all_test_files = Vec::new();

        for (subtask_idx, subtask) in self.subtasks.iter().enumerate() {
            self.print_progress((subtask_idx + 1) as i32, num_subtasks as i32, &format!("Subtask {}: {}", subtask_idx + 1, subtask.name));
            self.create_tests_for_subtask(subtask_idx, subtask, &mut global_test_id, &mut all_test_files, &solution_handles, solution_handle, &mut cpp_runner)?;
        }

        self.log_result("Running official solution:")?;
        let passed_subtasks = self.run_partial_solution(&all_test_files, &mut cpp_runner, solution_handle, self.solution_source.split('\n').count())?;
        // Every test was checked against the official solution as it was
        // generated, so this only fires when a run is not reproducible - a
        // solution sitting right on its time limit is the usual reason, and a
        // silent "Success!" would hide it.
        for (subtask_idx, subtask) in self.subtasks.iter().enumerate() {
            if !passed_subtasks.contains(&subtask_idx) && !all_test_files[subtask_idx].is_empty() {
                warn!(
                    "The official solution did not pass subtask {} ({}) when it was run on the finished tests.",
                    subtask_idx + 1,
                    subtask.name
                );
            }
        }

        for (i, partial) in solution_handles.iter().enumerate() {
            self.log_result(&format!("Running partial solution {}: {}", i + 1, self.solutions[i].name))?;
            let passed_subtasks = self.run_partial_solution(&all_test_files, &mut cpp_runner, *partial, self.solutions[i].source.split('\n').count())?;
            self.check_partial_solution_outcome(i, &passed_subtasks)?;
        }

        self.archive_tests(&all_test_files)?;

        let tests_size = fs_extra::dir::get_size(&self.tests_path).unwrap_or(0) as f32 / 1_000_000.0;
        self.log_result(&format!("Tests size: {}", style(format!("{tests_size:.2}MB")).bold()))?;

        // Log test counts per subtask
        for (i, tests) in all_test_files.iter().enumerate() {
            self.log_result(&format!("Subtask {}: {} tests", i + 1, tests.len()))?;
        }

        Ok(())
    }

    /// Rejects a partial solution that names a subtask the task does not have.
    ///
    /// Such an index is always a mistake (1-based numbering is the usual one),
    /// and it is a quiet one: the solution would simply be treated as one that
    /// has to fail everywhere, and the run would go on generating test data
    /// around a declaration that means nothing.
    fn check_declared_subtasks_exist(&self) -> Result<()> {
        for (partial_idx, solution) in self.solutions.iter().enumerate() {
            // Sorted, so the message does not depend on the iteration order of
            // the set behind it.
            let mut declared = solution.passes_subtasks.iter().copied().collect::<Vec<_>>();
            declared.sort_unstable();

            if let Some(&subtask_idx) = declared.iter().find(|&&idx| idx >= self.subtasks.len()) {
                return Err(Error::InvalidSubtaskIndex {
                    partial_number: partial_idx + 1,
                    partial_name: solution.name.clone(),
                    subtask_number: subtask_idx,
                    num_subtasks: self.subtasks.len(),
                });
            }
        }
        Ok(())
    }

    /// Checks the tests really do reject a partial solution everywhere it said
    /// it would fail.
    ///
    /// Test generation aims for this, but it can only report an error when it
    /// runs out of tries; without this check a task whose generators never
    /// produced a test that breaks a partial solution would still be reported as
    /// a success, and the subtask scores it hands out would be wrong.
    fn check_partial_solution_outcome(&self, partial_idx: usize, passed_subtasks: &HashSet<usize>) -> Result<()> {
        let solution = &self.solutions[partial_idx];

        for (subtask_idx, subtask) in self.subtasks.iter().enumerate() {
            if passed_subtasks.contains(&subtask_idx) && solution.should_fail(subtask_idx) {
                return Err(Error::PartialSolutionPassesExtraSubtask {
                    subtask_number: subtask_idx + 1,
                    partial_number: partial_idx + 1,
                    partial_name: solution.name.clone(),
                    subtask_name: subtask.name.clone(),
                });
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
