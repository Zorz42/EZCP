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

pub const DEFAULT_SEED: u64 = 0x455A_4350_5345_4544;
pub const DEFAULT_REPRODUCIBILITY_CHECKS: usize = 10;

static LOGGER_INIT: Once = Once::new();

/// `(test, subtask, test_in_subtask)`, all from zero.
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

pub struct Task<T: ToOutput> {
    pub(crate) name: String,
    pub(crate) problem_path: PathBuf,
    pub(crate) tests_path: PathBuf,
    pub(crate) tests_archive_path: PathBuf,
    pub(crate) build_folder_path: PathBuf,
    pub(crate) time_limit_ms: i32,
    pub(crate) memory_limit_mb: Option<usize>,
    pub(crate) seed: SeedChoice,
    /// `None` leaves it to the mode.
    pub(crate) reproducibility_checks: Option<usize>,
    pub(crate) input_file_name: FileNamer,
    pub(crate) output_file_name: FileNamer,
    pub(crate) subtasks: Vec<Subtask<T>>,
    pub(crate) solution_source: String,
    pub(crate) solutions: Vec<Solution>,
    pub(crate) min_failures_per_solution: usize,
    pub(crate) max_tries: usize,
    pub(crate) checker: Checker,
    pub(crate) trim_whitespace: bool,
    pub(crate) fail_fast: bool,
    pub(crate) jobs: Option<usize>,
    pub(crate) debug_level: LevelFilter,
    pub(crate) logger: MultiProgress,
}

fn diff_checker(_input: &str, official_output: &str, output: &str) -> bool {
    official_output.split_whitespace().eq(output.split_whitespace())
}

impl<T: ToOutput> Task<T> {
    /// The tests, the archive and the build folder go under `path`.
    #[must_use]
    pub fn new(name: &str, path: &Path) -> Self {
        Self {
            name: name.to_owned(),
            problem_path: path.to_owned(),
            tests_path: path.join("tests"),
            tests_archive_path: path.join("tests.zip"),
            build_folder_path: path.join("build"),
            time_limit_ms: 5000,
            memory_limit_mb: None,
            seed: SeedChoice::Fixed(DEFAULT_SEED),
            reproducibility_checks: None,
            input_file_name: Box::new(|test, subtask, _| format!("test.{:02}.{:03}.in", subtask + 1, test + 1)),
            output_file_name: Box::new(|test, subtask, _| format!("test.{:02}.{:03}.out", subtask + 1, test + 1)),
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

    #[must_use]
    pub fn with_solution_source(mut self, source: &str) -> Self {
        assert_eq!(self.solution_source, "", "the official solution was set twice");
        self.solution_source = source.to_owned();
        self
    }

    /// Called as `checker(input, official_output, output)`. The default compares whitespace-separated tokens.
    #[must_use]
    pub fn with_checker<F: Fn(&str, &str, &str) -> bool + Send + Sync + 'static>(mut self, checker: F) -> Self {
        self.checker = Box::new(checker);
        self
    }

    #[must_use]
    pub fn with_subtask(mut self, subtask: Subtask<T>) -> Self {
        self.subtasks.push(subtask);
        self
    }

    /// On by default: each run of whitespace in inputs and official outputs becomes one newline if it
    /// holds one and one space otherwise, and the text ends in a newline. Stubs are served with the
    /// setting they were written with.
    #[must_use]
    pub const fn trim_whitespace(mut self, trim_whitespace: bool) -> Self {
        self.trim_whitespace = trim_whitespace;
        self
    }

    /// The solution has to pass exactly `passes_subtasks` (counted from zero) and fail the others.
    #[must_use]
    pub fn with_partial_solution(mut self, name: &str, source: &str, passes_subtasks: &[usize]) -> Self {
        self.solutions.push(Solution {
            name: name.to_owned(),
            source: source.to_owned(),
            passes_subtasks: passes_subtasks.iter().copied().collect(),
        });
        self
    }

    /// How many tests of a subtask each partial solution meant to fail it has to fail (5). Zero turns
    /// the hunt off.
    #[must_use]
    pub const fn with_min_failures(mut self, n: usize) -> Self {
        self.min_failures_per_solution = n;
        self
    }

    /// How many candidates in a row may break nothing before the hunt gives up (100).
    #[must_use]
    pub const fn with_max_tries(mut self, n: usize) -> Self {
        self.max_tries = n;
        self
    }

    #[must_use]
    pub fn with_build_folder_path(mut self, path: PathBuf) -> Self {
        self.build_folder_path = path;
        self
    }

    /// Emptied by every run.
    #[must_use]
    pub fn with_tests_path(mut self, path: PathBuf) -> Self {
        self.tests_path = path;
        self
    }

    #[must_use]
    pub fn with_tests_archive_path(mut self, path: PathBuf) -> Self {
        self.tests_archive_path = path;
        self
    }

    /// Milliseconds of CPU time (5000).
    #[must_use]
    pub const fn with_time_limit(mut self, ms: i32) -> Self {
        self.time_limit_ms = ms;
        self
    }

    /// Megabytes of peak memory: resident on Linux and macOS, committed on Windows. None by default.
    #[must_use]
    pub const fn with_memory_limit(mut self, mb: usize) -> Self {
        self.memory_limit_mb = Some(mb);
        self
    }

    /// From `(test, subtask, test_in_subtask)`, all counted from zero. Names have to be distinct.
    #[must_use]
    pub fn with_input_file_name<F: Fn(usize, usize, usize) -> String + 'static>(mut self, f: F) -> Self {
        self.input_file_name = Box::new(f);
        self
    }

    #[must_use]
    pub fn with_output_file_name<F: Fn(usize, usize, usize) -> String + 'static>(mut self, f: F) -> Self {
        self.output_file_name = Box::new(f);
        self
    }

    /// The master seed. `--seed` overrides it.
    #[must_use]
    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = SeedChoice::Fixed(seed);
        self
    }

    /// A new master seed every run, written to `results.txt`.
    #[must_use]
    pub const fn with_random_seed(mut self) -> Self {
        self.seed = SeedChoice::Random;
        self
    }

    /// How many times each kept test is rebuilt from its seed and compared: by default 10 in seed mode
    /// and none otherwise.
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

    /// In the final run, stop a partial solution on a subtask it is meant to fail at its first failed
    /// test there; the rest are reported as skipped. `--fail-fast` turns it on.
    #[must_use]
    pub const fn with_fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    /// How many solutions run at once: by default one per CPU core, at most 4, as solutions competing
    /// for the machine get less reliable verdicts. The tests do not depend on it. `--jobs` overrides it.
    #[must_use]
    pub const fn with_jobs(mut self, jobs: usize) -> Self {
        assert!(jobs > 0, "at least one solution has to run at a time");
        self.jobs = Some(jobs);
        self
    }

    #[must_use]
    pub const fn with_debug_level(mut self, level: LevelFilter) -> Self {
        self.debug_level = level;
        self
    }

    pub(crate) fn limits(&self) -> Limits {
        Limits {
            time_ms: self.time_limit_ms,
            memory_kb: self.memory_limit_mb.map(|mb| mb as u64 * 1024),
        }
    }

    /// Runs in the mode the command line gives (`--help` lists it).
    pub fn run(mut self) -> Result<()> {
        // Before the logger exists.
        let options = CliOptions::from_env().inspect_err(|err| eprintln!("{err}\n\n{USAGE}"))?;
        if options.help {
            println!("{USAGE}");
            return Ok(());
        }
        self.jobs = options.jobs.or(self.jobs);
        self.fail_fast |= options.fail_fast;
        let seed = options.seed.unwrap_or(self.seed);
        self.run_with(options.mode, seed)
    }

    /// Ignores the command line.
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
        // Display width, for non-ASCII names.
        let border_text = format!(" {}", "=".repeat(console::measure_text_width(text) + 6));
        self.logger.println(&border_text).ok();
        self.logger.println(format!(" || {} ||", style(text).bold())).ok();
        self.logger.println(&border_text).ok();
    }
}
