use crate::solution::Solution;
use crate::subtask::Subtask;
use crate::{Error, Result};

use crate::archiver::archive_files;
use crate::create_tests::GeneratedTest;
use crate::logger_format::logger_format;
use crate::mode::{CliOptions, Mode, SeedChoice, USAGE};
use crate::progress::ScopedProgressBar;
use crate::rng::Rng;
use crate::runner::cpp_runner::CppRunner;
use crate::stub::{Part, Stub, stable_hash};
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

/// The master seed used when none is given, so that repeated runs produce the
/// same tests.
pub const DEFAULT_SEED: u64 = 0x455A_4350_5345_4544;

/// How many times seed mode rebuilds each finished test to check that its seed
/// reproduces it.
pub const DEFAULT_REPRODUCIBILITY_CHECKS: usize = 10;

pub static LOGGER_INIT: Once = Once::new();

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
    /// In milliseconds of CPU time.
    pub(crate) time_limit: i32,
    pub(crate) tests_archive_path: PathBuf,
    pub(crate) seed: SeedChoice,
    /// `None` leaves it to the mode, see [`Task::reproducibility_checks`].
    pub(crate) reproducibility_checks: Option<usize>,
    /// `(test_id, subtask_id, id_in_subtask) -> file name`
    pub(crate) get_input_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    pub(crate) get_output_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    pub(crate) build_folder_path: PathBuf,
    pub(crate) subtasks: Vec<Subtask<T>>,
    pub(crate) solution_source: String,
    /// The partial solutions.
    pub(crate) solutions: Vec<Solution>,
    pub(crate) min_failures_per_solution: usize,
    pub(crate) max_tries: usize,
    /// `(input, official_output, output) -> accepted`
    pub(crate) checker: fn(&str, &str, &str) -> bool,
    pub(crate) trim_whitespace: bool,
    pub(crate) debug_level: LevelFilter,
    pub(crate) logger: MultiProgress,
}

/// The input and output file names of a test.
type TestFiles = (String, String);

/// An unreadable directory counts as empty: this only feeds the size report.
fn dir_size(path: &Path) -> u64 {
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| {
            if entry.path().is_dir() {
                dir_size(&entry.path())
            } else {
                entry.metadata().map_or(0, |metadata| metadata.len())
            }
        })
        .sum()
}

fn format_size(bytes: u64) -> String {
    let bytes = bytes as f32;
    if bytes < 1_000_000.0 {
        format!("{:.1}kB", bytes / 1_000.0)
    } else {
        format!("{:.2}MB", bytes / 1_000_000.0)
    }
}

/// On Windows an antivirus scanner or the search indexer often holds one of the
/// old test files open for a moment, which makes a single attempt fail.
fn remove_dir_all_with_retry(path: &Path) -> Result<()> {
    for attempt in 1..5 {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(err) => debug!("Could not remove {} (attempt {attempt}): {err}", path.display()),
        }
        std::thread::sleep(std::time::Duration::from_millis(50 * attempt));
    }
    fs::remove_dir_all(path).map_err(Error::io(path))
}

/// Where two supposedly identical tests first differ, short enough for an error
/// message.
fn describe_difference(first: &str, second: &str) -> String {
    const EXCERPT_WIDTH: usize = 24;

    fn excerpt(text: &str, offset: usize) -> String {
        let bytes = text.as_bytes();
        let end = offset.saturating_add(EXCERPT_WIDTH).min(bytes.len());
        // The cut can split a multi-byte character.
        String::from_utf8_lossy(&bytes[offset.min(bytes.len())..end]).into_owned()
    }

    first.bytes().zip(second.bytes()).position(|(a, b)| a != b).map_or_else(
        || format!("one is {} bytes long and the other {}", first.len(), second.len()),
        |offset| {
            format!(
                "they first differ at byte {offset}, where the original has {:?} and the rebuilt one has {:?}",
                excerpt(first, offset),
                excerpt(second, offset)
            )
        },
    )
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
            seed: SeedChoice::Default,
            reproducibility_checks: None,
            get_input_file_name: Box::new(|test_id, subtask_id, _test_id_in_subtask| format!("test.{:02}.{:03}.in", subtask_id + 1, test_id + 1)),
            get_output_file_name: Box::new(|test_id, subtask_id, _test_id_in_subtask| format!("test.{:02}.{:03}.out", subtask_id + 1, test_id + 1)),
            build_folder_path: path.join("build"),
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

    fn results_file(&self) -> PathBuf {
        self.problem_path.join("results.txt")
    }

    pub(crate) fn log_result(&self, text: &str) -> Result<()> {
        let path = self.results_file();
        let mut file = OpenOptions::new().append(true).create(true).open(&path).map_err(Error::io(&path))?;
        writeln!(file, "{}", console::strip_ansi_codes(text)).map_err(Error::io(&path))?;
        info!("{text}");
        Ok(())
    }

    /// Sets the source code of the official solution.
    ///
    /// # Panics
    /// Panics if it is called a second time.
    #[must_use]
    pub fn with_solution_source(mut self, source: &str) -> Self {
        assert!(self.solution_source.is_empty());
        self.solution_source = source.to_owned();
        self
    }

    /// Sets the checker, called as `checker(input, official_output, output)`,
    /// for tasks with more than one correct answer. The default compares
    /// whitespace-separated tokens.
    #[must_use]
    pub fn with_checker(mut self, checker: fn(&str, &str, &str) -> bool) -> Self {
        self.checker = checker;
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
        self.solutions.push(Solution::new(name.to_owned(), source.to_owned(), passes_subtasks));
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

    /// Sets the directory the tests are written to.
    #[must_use]
    pub fn with_tests_path(mut self, path: PathBuf) -> Self {
        self.tests_path = path;
        self
    }

    /// Sets the time limit, in milliseconds of CPU time.
    #[must_use]
    pub const fn with_time_limit(mut self, limit: i32) -> Self {
        self.time_limit = limit;
        self
    }

    /// Sets the path of the tests archive.
    #[must_use]
    pub fn with_tests_archive_path(mut self, path: PathBuf) -> Self {
        self.tests_archive_path = path;
        self
    }

    /// Sets how input files are named, from `(test_id, subtask_id, id_in_subtask)`.
    /// Every test needs a distinct name.
    #[must_use]
    pub fn with_get_input_file_name<F: Fn(i32, i32, i32) -> String + 'static>(mut self, f: F) -> Self {
        self.get_input_file_name = Box::new(f);
        self
    }

    /// Sets how output files are named, like [`Task::with_get_input_file_name`].
    #[must_use]
    pub fn with_get_output_file_name<F: Fn(i32, i32, i32) -> String + 'static>(mut self, f: F) -> Self {
        self.get_output_file_name = Box::new(f);
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
    /// [`DEFAULT_REPRODUCIBILITY_CHECKS`] times; zero turns it off.
    #[must_use]
    pub const fn with_reproducibility_checks(mut self, times: usize) -> Self {
        self.reproducibility_checks = Some(times);
        self
    }

    const fn reproducibility_checks(&self, mode: Mode) -> usize {
        match self.reproducibility_checks {
            Some(times) => times,
            None if matches!(mode, Mode::Seeds) => DEFAULT_REPRODUCIBILITY_CHECKS,
            None => 0,
        }
    }

    /// Sets the log level.
    #[must_use]
    pub const fn with_debug_level(mut self, level: LevelFilter) -> Self {
        self.debug_level = level;
        self
    }

    /// Runs the task in the [`Mode`] given on the command line (`--help` lists
    /// the options). Unknown arguments are an error.
    pub fn run(self) -> Result<()> {
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
        self.create_tests_inner(mode, seed.resolve(DEFAULT_SEED)).inspect_err(log_error)?;
        info!("Elapsed time: {}", style(format!("{:.2}s", start_time.elapsed().as_secs_f32())).bold());
        self.logger.println(style("Success!").green().bright().bold().to_string()).ok();
        Ok(())
    }

    fn print_title(&self, text: &str) {
        // Display width rather than byte length, for non-ASCII task names.
        let border_text = format!(" {}", "=".repeat(console::measure_text_width(text) + 6));
        self.logger.println(&border_text).ok();
        self.logger.println(format!(" || {} ||", style(text).bold())).ok();
        self.logger.println(&border_text).ok();
    }

    fn create_tests_inner(&self, mode: Mode, seed: u64) -> Result<()> {
        self.logger.println("").ok();
        self.print_title(&format!("Creating tests for task \"{}\"", self.name));

        if self.subtasks.is_empty() {
            warn!("No subtasks defined.");
        }
        self.check_declared_subtasks_exist()?;
        if self.solution_source.is_empty() {
            return Err(Error::MissingSolution);
        }

        let mut cpp_runner = CppRunner::new(&self.build_folder_path)?;
        let solution_handle = cpp_runner.add_program(&self.solution_source)?;
        let solution_handles = self.solutions.iter().map(|solution| cpp_runner.add_program(&solution.source)).collect::<Result<Vec<_>>>()?;
        // Only after every program has been added: anything else is stale.
        cpp_runner.clean_build_folder()?;

        if self.tests_path.exists() {
            remove_dir_all_with_retry(&self.tests_path)?;
        }
        fs::create_dir_all(&self.tests_path).map_err(Error::io(&self.tests_path))?;
        fs::write(self.results_file(), "").map_err(Error::io(&self.results_file()))?;

        self.log_result(&format!("Master seed: {}", style(format!("{seed:#018x}")).bold()))?;
        let mut rng = Rng::from_seed(seed);
        let mut all_tests = Vec::new();
        for (subtask_idx, subtask) in self.subtasks.iter().enumerate() {
            let progress = format!("[{}/{}]", style(subtask_idx + 1).bold(), style(self.subtasks.len()).bold());
            self.logger
                .println(format!("{progress} {}", style(format!("Subtask {}: {}", subtask_idx + 1, subtask.name)).cyan().bold()))
                .ok();
            all_tests.push(self.create_tests_for_subtask(subtask_idx, subtask, &mut rng, &solution_handles, solution_handle, &mut cpp_runner)?);
        }

        self.check_tests_are_reproducible(self.reproducibility_checks(mode), &all_tests)?;
        let names = self.assign_file_names(&all_tests)?;

        self.log_result("Running official solution:")?;
        let passed_subtasks = self.run_partial_solution(&all_tests, &mut cpp_runner, solution_handle, self.solution_source.split('\n').count())?;
        self.warn_about_unreproduced_passes("The official solution", |_subtask_idx| true, &passed_subtasks, &all_tests);

        for (i, (solution, &handle)) in self.solutions.iter().zip(&solution_handles).enumerate() {
            self.log_result(&format!("Running partial solution {}: {}", i + 1, solution.name))?;
            let passed_subtasks = self.run_partial_solution(&all_tests, &mut cpp_runner, handle, solution.source.split('\n').count())?;
            // Generation only errors out when it runs out of tries, so this is checked again.
            if let Some((subtask_idx, subtask)) = self.subtasks.iter().enumerate().find(|&(idx, _)| passed_subtasks.contains(&idx) && solution.should_fail(idx)) {
                return Err(Error::PartialSolutionPassesExtraSubtask {
                    subtask_number: subtask_idx + 1,
                    partial_number: i + 1,
                    partial_name: solution.name.clone(),
                    subtask_name: subtask.name.clone(),
                });
            }
            self.warn_about_unreproduced_passes(
                &format!("Partial solution {} ({})", i + 1, solution.name),
                |idx| !solution.should_fail(idx),
                &passed_subtasks,
                &all_tests,
            );
        }

        // Written only once everything is verified, so a failed run leaves no tests.
        self.write_tests(mode, &names, &all_tests)?;
        let files: Vec<_> = names.iter().flat_map(|(input, output)| [input, output]).map(|name| self.tests_path.join(name)).collect();
        archive_files(&files, &self.tests_archive_path, &self.logger)?;

        self.log_result(&format!("Tests size: {}", style(format_size(dir_size(&self.tests_path))).bold()))?;
        if mode == Mode::Seeds {
            self.log_result("The test files are seeds: pipe one into the task with --serve to rebuild it")?;
        }
        for (i, tests) in all_tests.iter().enumerate() {
            self.log_result(&format!("Subtask {}: {} tests", i + 1, tests.len()))?;
        }
        Ok(())
    }

    /// Rebuilds every kept test's input from its seed `times` over. Nothing can
    /// stop a generator from using randomness other than its `Rng`, so this is
    /// how such a generator is caught before its seeds are written down.
    fn check_tests_are_reproducible(&self, times: usize, all_tests: &[Vec<GeneratedTest>]) -> Result<()> {
        let total_tests: usize = all_tests.iter().map(Vec::len).sum();
        if times == 0 || total_tests == 0 {
            return Ok(());
        }

        info!("Checking that all {total_tests} tests can be rebuilt from their seeds ({times} times each)");
        let progress_bar = ScopedProgressBar::new(&self.logger, (total_tests * times) as u64);
        for (subtask_idx, test) in with_subtasks(all_tests) {
            for attempt in 1..=times {
                let rebuilt = self.generate_input(subtask_idx, test.generator, test.seed);
                progress_bar.inc(1);
                if rebuilt != *test.input {
                    return Err(Error::GeneratorNotReproducible {
                        subtask_number: subtask_idx + 1,
                        gen_id: test.generator + 1,
                        seed: format!("{:#018x}", test.seed),
                        attempt,
                        attempts: times,
                        details: describe_difference(&test.input, &rebuilt),
                    });
                }
            }
        }
        Ok(())
    }

    fn assign_file_names(&self, all_tests: &[Vec<GeneratedTest>]) -> Result<Vec<TestFiles>> {
        let mut names: Vec<TestFiles> = Vec::new();
        // The naming closures come from the user and may map two tests to one name.
        let mut used_names = HashSet::new();
        for (subtask_idx, subtask_tests) in all_tests.iter().enumerate() {
            for id_in_subtask in 0..subtask_tests.len() {
                let ids = (names.len() as i32, subtask_idx as i32, id_in_subtask as i32);
                let files = ((self.get_input_file_name)(ids.0, ids.1, ids.2), (self.get_output_file_name)(ids.0, ids.1, ids.2));
                if let Some(name) = [&files.0, &files.1].into_iter().find(|&name| !used_names.insert(name.clone())) {
                    return Err(Error::TestAlreadyExists { path: name.clone() });
                }
                names.push(files);
            }
        }
        Ok(names)
    }

    /// Writes each test's data, or in seed mode the stub that rebuilds it.
    fn write_tests(&self, mode: Mode, names: &[TestFiles], all_tests: &[Vec<GeneratedTest>]) -> Result<()> {
        for ((input_name, output_name), (subtask, test)) in names.iter().zip(with_subtasks(all_tests)) {
            for (name, part, contents) in [(input_name, Part::Input, &test.input), (output_name, Part::Output, &test.output)] {
                let contents = if mode == Mode::Files {
                    contents.to_string()
                } else {
                    Stub {
                        subtask,
                        generator: test.generator,
                        seed: test.seed,
                        part,
                        hash: Some(stable_hash(contents)),
                    }
                    .to_line()
                };
                let path = self.tests_path.join(name);
                fs::write(&path, contents).map_err(Error::io(&path))?;
            }
        }
        Ok(())
    }

    /// Otherwise a wrong index (usually 1-based) would silently make the
    /// solution one that has to fail everywhere.
    fn check_declared_subtasks_exist(&self) -> Result<()> {
        for (partial_idx, solution) in self.solutions.iter().enumerate() {
            // The smallest, so the error does not depend on hash set order.
            if let Some(&subtask_number) = solution.passes_subtasks.iter().filter(|&&idx| idx >= self.subtasks.len()).min() {
                return Err(Error::InvalidSubtaskIndex {
                    partial_number: partial_idx + 1,
                    partial_name: solution.name.clone(),
                    subtask_number,
                    num_subtasks: self.subtasks.len(),
                });
            }
        }
        Ok(())
    }

    /// Every test was already checked against these solutions during
    /// generation, so a failure here means the run is not reproducible, usually
    /// because a solution is right at the time limit.
    fn warn_about_unreproduced_passes(&self, solution: &str, meant_to_pass: impl Fn(usize) -> bool, passed_subtasks: &HashSet<usize>, all_tests: &[Vec<GeneratedTest>]) {
        for (subtask_idx, subtask) in self.subtasks.iter().enumerate() {
            if meant_to_pass(subtask_idx) && !passed_subtasks.contains(&subtask_idx) && !all_tests[subtask_idx].is_empty() {
                warn!("{solution} did not pass subtask {} ({}) when it was run on the finished tests.", subtask_idx + 1, subtask.name);
            }
        }
    }
}

/// Every test, in order, with the index of its subtask.
fn with_subtasks(all_tests: &[Vec<GeneratedTest>]) -> impl Iterator<Item = (usize, &GeneratedTest)> {
    all_tests.iter().enumerate().flat_map(|(subtask_idx, tests)| tests.iter().map(move |test| (subtask_idx, test)))
}
