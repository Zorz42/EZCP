use crate::logger_format::logger_format;
use crate::mode::{CliOptions, Mode, SeedChoice, USAGE};
use crate::runner::exec_runner::Limits;
use crate::subtask::Subtask;
use crate::to_output::ToOutput;
use crate::{Error, Result};
use console::style;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::{LevelFilter, error, info};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;

/// The master seed used when none is given, so that repeated runs produce the
/// same tests.
pub const DEFAULT_SEED: u64 = 0x455A_4350_5345_4544;

/// How many times seed mode rebuilds each finished test to check that its seed
/// reproduces it.
pub const DEFAULT_REPRODUCIBILITY_CHECKS: usize = 10;

pub static LOGGER_INIT: Once = Once::new();

/// `(test, subtask, test_in_subtask) -> file name`, all counted from zero.
type FileNamer = Box<dyn Fn(usize, usize, usize) -> String>;

/// `(input, official_output, output) -> accepted`
type Checker = Box<dyn Fn(&str, &str, &str) -> bool + Send + Sync>;

/// A partial solution.
pub struct Solution {
    pub name: String,
    pub source: String,
    pub passes_subtasks: HashSet<usize>,
}

impl Solution {
    pub fn should_fail(&self, subtask: usize) -> bool {
        !self.passes_subtasks.contains(&subtask)
    }
}

/// A competitive programming task: its solutions, subtasks and generation
/// settings.
///
/// For each subtask, test generation keeps going until every partial solution
/// meant to fail it has failed `min_failures` of its tests, or `max_tries`
/// candidates in a row have added nothing.
pub struct Task<T: ToOutput> {
    pub(crate) name: String,
    pub(crate) problem_path: PathBuf,
    pub(crate) tests_path: PathBuf,
    pub(crate) tests_archive_path: PathBuf,
    pub(crate) build_folder_path: PathBuf,
    /// In milliseconds of CPU time.
    pub(crate) time_limit: i32,
    /// In megabytes. `None` is no limit.
    pub(crate) memory_limit: Option<usize>,
    pub(crate) seed: SeedChoice,
    /// `None` leaves it to the mode, see [`Task::reproducibility_checks`].
    pub(crate) reproducibility_checks: Option<usize>,
    pub(crate) input_file_name: FileNamer,
    pub(crate) output_file_name: FileNamer,
    pub(crate) subtasks: Vec<Subtask<T>>,
    pub(crate) solution_source: String,
    /// The partial solutions.
    pub(crate) solutions: Vec<Solution>,
    pub(crate) min_failures_per_solution: usize,
    pub(crate) max_tries: usize,
    pub(crate) checker: Checker,
    pub(crate) trim_whitespace: bool,
    pub(crate) fail_fast: bool,
    /// How many solutions run at once. `None` leaves it to the runner.
    pub(crate) jobs: Option<usize>,
    pub(crate) debug_level: LevelFilter,
    pub(crate) logger: MultiProgress,
}

fn diff_checker(_test_input: &str, official_output: &str, program_output: &str) -> bool {
    official_output.split_whitespace().eq(program_output.split_whitespace())
}

impl<T: ToOutput> Task<T> {
    /// Creates a task whose tests, archive and build files go under `path`.
    #[must_use]
    pub fn new(name: &str, path: &Path) -> Self {
        Self {
            name: name.to_owned(),
            problem_path: path.to_owned(),
            tests_path: path.join("tests"),
            tests_archive_path: path.join("tests.zip"),
            build_folder_path: path.join("build"),
            time_limit: 5000,
            memory_limit: None,
            seed: SeedChoice::Fixed(DEFAULT_SEED),
            reproducibility_checks: None,
            input_file_name: Box::new(|test, subtask, _test_in_subtask| format!("test.{:02}.{:03}.in", subtask + 1, test + 1)),
            output_file_name: Box::new(|test, subtask, _test_in_subtask| format!("test.{:02}.{:03}.out", subtask + 1, test + 1)),
            subtasks: Vec::new(),
            solution_source: String::new(),
            solutions: Vec::new(),
            min_failures_per_solution: 5,
            max_tries: 100,
            checker: Box::new(diff_checker),
            trim_whitespace: true,
            fail_fast: false,
            jobs: None,
            debug_level: LevelFilter::Info,
            logger: MultiProgress::new(),
        }
    }

    /// Sets the source code of the official solution.
    ///
    /// # Panics
    /// Panics if it is called a second time.
    #[must_use]
    pub fn with_solution_source(mut self, source: &str) -> Self {
        assert_eq!(self.solution_source, "", "the official solution was set twice");
        self.solution_source = source.to_owned();
        self
    }

    /// Sets the checker, called as `checker(input, official_output, output)`,
    /// for tasks with more than one correct answer. The default compares
    /// whitespace-separated tokens.
    #[must_use]
    pub fn with_checker<F: Fn(&str, &str, &str) -> bool + Send + Sync + 'static>(mut self, checker: F) -> Self {
        self.checker = Box::new(checker);
        self
    }

    /// Adds a subtask.
    #[must_use]
    pub fn with_subtask(mut self, subtask: Subtask<T>) -> Self {
        self.subtasks.push(subtask);
        self
    }

    /// Whether to normalise the whitespace of inputs and official outputs: each
    /// run of whitespace becomes one newline if it contains one and one space
    /// otherwise, and the text ends in a single newline. On by default.
    ///
    /// Stubs have to be served with the same setting they were written with.
    #[must_use]
    pub const fn trim_whitespace(mut self, trim_whitespace: bool) -> Self {
        self.trim_whitespace = trim_whitespace;
        self
    }

    /// Adds a partial solution that is expected to pass exactly the subtasks
    /// in `passes_subtasks` (0-based) and fail all others.
    #[must_use]
    pub fn with_partial_solution(mut self, name: &str, source: &str, passes_subtasks: &[usize]) -> Self {
        self.solutions.push(Solution {
            name: name.to_owned(),
            source: source.to_owned(),
            passes_subtasks: passes_subtasks.iter().copied().collect(),
        });
        self
    }

    /// Sets how many tests of a subtask each partial solution meant to fail it
    /// has to fail. One test counts for every solution it breaks; zero turns the
    /// search off.
    #[must_use]
    pub const fn with_min_failures(mut self, n: usize) -> Self {
        self.min_failures_per_solution = n;
        self
    }

    /// Sets how many candidates in a row may break no partial solution before
    /// the search for more tests gives up.
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

    /// Sets the directory the tests are written to. It is emptied by every run.
    #[must_use]
    pub fn with_tests_path(mut self, path: PathBuf) -> Self {
        self.tests_path = path;
        self
    }

    /// Sets the path of the tests archive.
    #[must_use]
    pub fn with_tests_archive_path(mut self, path: PathBuf) -> Self {
        self.tests_archive_path = path;
        self
    }

    /// Sets the time limit, in milliseconds of CPU time.
    #[must_use]
    pub const fn with_time_limit(mut self, limit: i32) -> Self {
        self.time_limit = limit;
        self
    }

    /// Sets the memory limit, in megabytes. There is none by default.
    ///
    /// A solution is over the limit if its peak memory use is: resident memory
    /// on Linux and macOS, committed memory on Windows.
    #[must_use]
    pub const fn with_memory_limit(mut self, megabytes: usize) -> Self {
        self.memory_limit = Some(megabytes);
        self
    }

    /// Sets how input files are named, from `(test, subtask, test_in_subtask)`,
    /// all counted from zero. Every test needs a distinct name.
    #[must_use]
    pub fn with_input_file_name<F: Fn(usize, usize, usize) -> String + 'static>(mut self, f: F) -> Self {
        self.input_file_name = Box::new(f);
        self
    }

    /// Sets how output files are named, like [`Task::with_input_file_name`].
    #[must_use]
    pub fn with_output_file_name<F: Fn(usize, usize, usize) -> String + 'static>(mut self, f: F) -> Self {
        self.output_file_name = Box::new(f);
        self
    }

    /// Sets the master seed. `--seed` on the command line overrides it.
    #[must_use]
    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = SeedChoice::Fixed(seed);
        self
    }

    /// Draws a new master seed on every run. It is written to `results.txt`.
    #[must_use]
    pub const fn with_random_seed(mut self) -> Self {
        self.seed = SeedChoice::Random;
        self
    }

    /// Sets how many times each finished test's input is rebuilt from its seed
    /// and compared, in every mode. By default only seed mode checks,
    /// 10 times; zero turns it off.
    #[must_use]
    pub const fn with_reproducibility_checks(mut self, times: usize) -> Self {
        self.reproducibility_checks = Some(times);
        self
    }

    pub(crate) const fn reproducibility_checks(&self, mode: Mode) -> usize {
        match self.reproducibility_checks {
            Some(times) => times,
            None if matches!(mode, Mode::Seeds) => DEFAULT_REPRODUCIBILITY_CHECKS,
            None => 0,
        }
    }

    /// Whether to stop running a partial solution on a subtask it is meant to
    /// fail at its first failed test there, when the finished tests are run
    /// through every solution. `--fail-fast` on the command line turns it on.
    ///
    /// A partial solution that is too slow otherwise spends the whole time
    /// limit on every one of those tests. The results then report the tests
    /// after the first failed one as skipped.
    #[must_use]
    pub const fn with_fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    /// Sets how many solutions run at once. By default that is the number of
    /// CPU cores, but at most four, since solutions that compete for the
    /// machine get less reliable verdicts. `--jobs` on the command line
    /// overrides it.
    ///
    /// The generated tests do not depend on it.
    ///
    /// # Panics
    /// Panics if `jobs` is zero.
    #[must_use]
    pub const fn with_jobs(mut self, jobs: usize) -> Self {
        assert!(jobs > 0, "at least one solution has to run at a time");
        self.jobs = Some(jobs);
        self
    }

    /// Sets the log level.
    #[must_use]
    pub const fn with_debug_level(mut self, level: LevelFilter) -> Self {
        self.debug_level = level;
        self
    }

    pub(crate) fn limits(&self) -> Limits {
        Limits {
            time_ms: self.time_limit,
            memory_kb: self.memory_limit.map(|megabytes| megabytes as u64 * 1024),
        }
    }

    /// Runs the task in the [`Mode`] given on the command line (`--help` lists
    /// the options). Unknown arguments are an error.
    pub fn run(mut self) -> Result<()> {
        let options = match CliOptions::from_env() {
            Ok(options) => options,
            Err(err) => {
                // The logger is not set up yet.
                eprintln!("{err}\n\n{USAGE}");
                return Err(err);
            }
        };

        if options.help {
            println!("{USAGE}");
            return Ok(());
        }

        self.jobs = options.jobs.or(self.jobs);
        self.fail_fast |= options.fail_fast;
        let seed = options.seed.unwrap_or(self.seed);
        self.run_with(options.mode, seed)
    }

    /// Runs the task in `mode`, ignoring the command line.
    pub fn run_mode(self, mode: Mode) -> Result<()> {
        let seed = self.seed;
        self.run_with(mode, seed)
    }

    fn run_with(self, mode: Mode, seed: SeedChoice) -> Result<()> {
        LOGGER_INIT.call_once(|| {
            let logger = env_logger::builder().filter(None, self.debug_level).format(logger_format).build();
            LogWrapper::new(self.logger.clone(), logger).try_init().ok();
            log::set_max_level(self.debug_level);
        });
        let log_error = |err: &Error| error!("{}", style(err).red().bright());

        if mode == Mode::Serve {
            return self.serve().inspect_err(log_error);
        }

        let start_time = std::time::Instant::now();
        self.create_tests(mode, seed.resolve()).inspect_err(log_error)?;
        info!("Elapsed time: {}", style(format!("{:.2}s", start_time.elapsed().as_secs_f32())).bold());
        self.logger.println(style("Success!").green().bright().bold().to_string()).ok();
        Ok(())
    }

    pub(crate) fn results_file(&self) -> PathBuf {
        self.problem_path.join("results.txt")
    }

    /// Logs `text` and appends it to the results file.
    pub(crate) fn log_result(&self, text: &str) -> Result<()> {
        let path = self.results_file();
        let mut file = OpenOptions::new().append(true).create(true).open(&path).map_err(Error::io(&path))?;
        writeln!(file, "{}", console::strip_ansi_codes(text)).map_err(Error::io(&path))?;
        info!("{text}");
        Ok(())
    }

    pub(crate) fn print_title(&self, text: &str) {
        // Display width rather than byte length, for non-ASCII task names.
        let border_text = format!(" {}", "=".repeat(console::measure_text_width(text) + 6));
        self.logger.println(&border_text).ok();
        self.logger.println(format!(" || {} ||", style(text).bold())).ok();
        self.logger.println(&border_text).ok();
    }
}
