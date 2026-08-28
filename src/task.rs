use crate::solution::Solution;
use crate::subtask::Subtask;
use crate::{Error, Result};

use crate::archiver::archive_files;
use crate::create_tests::GeneratedTest;
use crate::logger_format::logger_format;
use crate::manifest::{Manifest, ManifestSubtask, ManifestTest, stable_hash};
use crate::mode::{CliOptions, Mode, SeedChoice, USAGE};
use crate::progress::ScopedProgressBar;
use crate::rng::Rng;
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

/// The master seed a task uses when none is given.
///
/// It is a constant rather than something drawn from the clock so that running a
/// task twice produces the same test data twice. Pass `--seed random` (or
/// [`Task::with_random_seed`]) to explore new tests instead; the seed that was
/// used is always recorded in the manifest.
pub const DEFAULT_SEED: u64 = 0x455A_4350_5345_4544;

/// How many times seed mode rebuilds each finished test to prove it is
/// reproducible.
///
/// Seed mode keeps nothing but the recipe for a test, so a generator that is not
/// reproducible costs the whole run: there is no file to fall back on, and the
/// mistake would only surface much later, when a judge asked for a test and got
/// something else. Ten rebuilds is cheap next to generating and judging the tests
/// in the first place, and it catches a generator that is only occasionally
/// unfaithful as well as one that never repeats itself.
pub const DEFAULT_REPRODUCIBILITY_CHECKS: usize = 10;

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
    /// Path of the seed manifest, which records how every test was made
    pub(crate) manifest_path: PathBuf,
    /// Where the master seed for test generation comes from
    pub(crate) seed: SeedChoice,
    /// How many times each finished test is rebuilt to check it comes out the
    /// same. `None` leaves it to the mode: see [`Task::reproducibility_checks`].
    pub(crate) reproducibility_checks: Option<usize>,
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

/// Describes how two supposedly identical tests differ, briefly enough to sit in
/// an error message.
///
/// The point of failure is what matters when a generator turns out to be
/// unfaithful, so this shows where the two first part ways rather than dumping
/// two tests that may be megabytes each.
fn describe_difference(first: &str, second: &str) -> String {
    /// How much of each version to quote around the first difference.
    const EXCERPT_WIDTH: usize = 24;

    fn excerpt(text: &str, offset: usize) -> String {
        let bytes = text.as_bytes();
        let end = offset.saturating_add(EXCERPT_WIDTH).min(bytes.len());
        // Lossy, because the cut can land in the middle of a multi-byte
        // character and this is only ever shown to a person.
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
            manifest_path: path.join("seeds.json"),
            seed: SeedChoice::Default,
            reproducibility_checks: None,
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

    /// Adds a subtask to the task.
    #[must_use]
    pub fn with_subtask(mut self, subtask: Subtask<T>) -> Self {
        self.subtasks.push(subtask);
        self
    }

    /// Trims trailing whitespace from each line of a solution's output, and
    /// trailing blank lines from the end of it. On by default.
    ///
    /// This changes the bytes of the test data, so the setting is recorded in the
    /// manifest and applies identically to a served test.
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

    /// Sets the path of the seed manifest.
    ///
    /// The manifest is what [seed mode](Mode::Seeds) writes instead of test
    /// files, and what [`--serve`](Mode::Serve) reads to know which tests exist.
    #[must_use]
    pub fn with_manifest_path(mut self, path: PathBuf) -> Self {
        self.manifest_path = path;
        self
    }

    /// Sets the master seed for test generation.
    ///
    /// Two runs with the same seed generate the same tests, so this is how a set
    /// of test data is pinned down. A `--seed` on the command line overrides it.
    #[must_use]
    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = SeedChoice::Fixed(seed);
        self
    }

    /// Draws a fresh master seed on every run.
    ///
    /// Useful while a task is being written, when the point is to keep looking
    /// for tests that break a partial solution rather than to reproduce an
    /// earlier run. The seed that was drawn is written to the manifest, so a run
    /// worth keeping can be repeated with [`Task::with_seed`].
    #[must_use]
    pub const fn with_random_seed(mut self) -> Self {
        self.seed = SeedChoice::Random;
        self
    }

    /// Sets how many times each finished test is rebuilt from its seed to check
    /// that it comes out the same.
    ///
    /// [Seed mode](Mode::Seeds) does this on its own, [`DEFAULT_REPRODUCIBILITY_CHECKS`]
    /// times; file mode does not, because it has the tests themselves. Setting a
    /// count applies it to both, and zero turns the check off.
    ///
    /// Only the inputs are rebuilt. The official solution is not run again: its
    /// output is a function of its input, so an input that comes back identical
    /// brings the same output with it.
    #[must_use]
    pub const fn with_reproducibility_checks(mut self, times: usize) -> Self {
        self.reproducibility_checks = Some(times);
        self
    }

    /// How many rebuilds this run should do, given the mode it is running in.
    const fn reproducibility_checks(&self, mode: Mode) -> usize {
        match self.reproducibility_checks {
            Some(times) => times,
            None if matches!(mode, Mode::Seeds) => DEFAULT_REPRODUCIBILITY_CHECKS,
            // File mode keeps the tests, so a generator that cannot rebuild them
            // costs nothing until somebody serves them - and the server checks
            // every test against the hash in the manifest anyway.
            None => 0,
        }
    }

    /// Sets the log level for output.
    #[must_use]
    pub const fn with_debug_level(mut self, level: LevelFilter) -> Self {
        self.debug_level = level;
        self
    }

    /// Runs the task, taking the mode from the command line.
    ///
    /// With no arguments this compiles the solutions, generates the tests, writes
    /// them out and archives them, as it always has. `--seeds` keeps only the
    /// manifest and `--serve` answers requests for tests on stdin; `--help`
    /// describes them. See [`Mode`] for what each one does.
    ///
    /// A task binary is what an online judge invokes, so an argument that is not
    /// recognised is an error rather than something to ignore. Call
    /// [`Task::run_mode`] instead to choose the mode in code and leave the
    /// command line alone.
    pub fn run(self) -> Result<()> {
        let options = match CliOptions::from_env() {
            Ok(options) => options,
            Err(err) => {
                // The logger is not up yet, and this is a usage error rather than
                // a failure of the task, so it goes straight to the terminal.
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

    /// Runs the task in a given mode, ignoring the command line.
    pub fn run_mode(self, mode: Mode) -> Result<()> {
        let seed = self.seed;
        self.run_with(mode, seed)
    }

    /// Sets up logging and dispatches to whichever mode was asked for.
    fn run_with(self, mode: Mode, seed: SeedChoice) -> Result<()> {
        LOGGER_INIT.call_once(|| {
            let mut builder = env_logger::builder();
            builder.filter(None, self.debug_level);
            builder.format(logger_format);
            let env_logger_instance = builder.build();

            LogWrapper::new(self.logger.clone(), env_logger_instance).try_init().ok();
            log::set_max_level(self.debug_level);
            debug!("Logger initialized with level: {}", self.debug_level);
        });

        // Serving is a long-running process answering requests, not a build, so
        // none of the timing or the "Success!" banner belongs to it.
        if mode == Mode::Serve {
            return self.serve().inspect_err(|err| error!("{}", style(err).red().bright()));
        }

        let start_time = std::time::Instant::now();
        let res = self.create_tests_inner(mode, seed.resolve(DEFAULT_SEED));
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

    /// Compiles the solutions, generates every test and verifies the outcome.
    ///
    /// The two generating modes share all of this: the tests are produced and
    /// checked identically, and the mode only decides what is kept afterwards.
    fn create_tests_inner(&self, mode: Mode, seed: u64) -> Result<()> {
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
        if mode == Mode::Files {
            if self.tests_path.exists() {
                remove_dir_all_with_retry(&self.tests_path)?;
            }
            fs::create_dir_all(&self.tests_path).map_err(|err| Error::IOError {
                err,
                file: path_str(&self.tests_path),
            })?;
        }

        // clear log file
        fs::File::create(self.get_results_file()).map_err(|e| Error::IOError {
            err: e,
            file: path_str(&self.get_results_file()),
        })?;

        info!("Master seed: {}", style(format!("{seed:#018x}")).bold());
        // One generator drives the whole run, so the seed alone decides every test
        // that gets generated.
        let mut rng = Rng::from_seed(seed);

        let num_subtasks = self.subtasks.len();
        let mut all_tests = Vec::new();

        for (subtask_idx, subtask) in self.subtasks.iter().enumerate() {
            self.print_progress((subtask_idx + 1) as i32, num_subtasks as i32, &format!("Subtask {}: {}", subtask_idx + 1, subtask.name));
            all_tests.push(self.create_tests_for_subtask(subtask_idx, subtask, &mut rng, &solution_handles, solution_handle, &mut cpp_runner)?);
        }

        // Before anything is kept or judged: prove that the tests just generated
        // can be built again from the seeds that are about to be recorded.
        self.check_tests_are_reproducible(self.reproducibility_checks(mode), &all_tests)?;

        let manifest = self.build_manifest(seed, &all_tests)?;

        if mode == Mode::Files {
            self.write_tests(&manifest, &all_tests)?;
        }

        self.log_result("Running official solution:")?;
        let passed_subtasks = self.run_partial_solution(&all_tests, &mut cpp_runner, solution_handle, self.solution_source.split('\n').count())?;
        // Every test was checked against the official solution as it was
        // generated, so this only fires when a run is not reproducible - a
        // solution sitting right on its time limit is the usual reason, and a
        // silent "Success!" would hide it.
        for (subtask_idx, subtask) in self.subtasks.iter().enumerate() {
            if !passed_subtasks.contains(&subtask_idx) && !all_tests[subtask_idx].is_empty() {
                warn!(
                    "The official solution did not pass subtask {} ({}) when it was run on the finished tests.",
                    subtask_idx + 1,
                    subtask.name
                );
            }
        }

        for (i, partial) in solution_handles.iter().enumerate() {
            self.log_result(&format!("Running partial solution {}: {}", i + 1, self.solutions[i].name))?;
            let passed_subtasks = self.run_partial_solution(&all_tests, &mut cpp_runner, *partial, self.solutions[i].source.split('\n').count())?;
            self.check_partial_solution_outcome(i, &passed_subtasks)?;
        }

        // Written last, so a manifest on disk always describes a set of tests that
        // was verified all the way through.
        manifest.write(&self.manifest_path)?;

        if mode == Mode::Files {
            self.archive_tests(&manifest)?;

            let tests_size = fs_extra::dir::get_size(&self.tests_path).unwrap_or(0) as f32 / 1_000_000.0;
            self.log_result(&format!("Tests size: {}", style(format!("{tests_size:.2}MB")).bold()))?;
        } else {
            let manifest_size = fs::metadata(&self.manifest_path).map_or(0, |metadata| metadata.len()) as f32 / 1_000.0;
            self.log_result(&format!(
                "{} tests kept as seeds in {} ({})",
                manifest.num_tests(),
                path_str(&self.manifest_path),
                style(format!("{manifest_size:.1}kB")).bold()
            ))?;
        }

        // Log test counts per subtask
        for (i, tests) in all_tests.iter().enumerate() {
            self.log_result(&format!("Subtask {}: {} tests", i + 1, tests.len()))?;
        }

        Ok(())
    }

    /// Rebuilds every finished test from its seed `times` over and checks that
    /// it comes out identical each time.
    ///
    /// A seed is only worth recording if it really does reproduce the test. The
    /// framework cannot stop a generator from reaching for randomness it was not
    /// given - a `rand::rng()` call, the clock, a value captured when the task was
    /// described, the iteration order of a `HashMap` - so instead it tries the
    /// thing that would go wrong and refuses to write a manifest that lies.
    ///
    /// Only the finished tests are rebuilt, not the candidates that were thrown
    /// away along the way, and only their inputs: the official solution's output
    /// follows from its input.
    fn check_tests_are_reproducible(&self, times: usize, all_tests: &[Vec<GeneratedTest>]) -> Result<()> {
        let total_tests: usize = all_tests.iter().map(Vec::len).sum();
        if times == 0 || total_tests == 0 {
            return Ok(());
        }

        info!("Checking that all {total_tests} tests can be rebuilt from their seeds ({times} times each)");
        let progress_bar = ScopedProgressBar::new(&self.logger, (total_tests * times) as u64);

        for (subtask_idx, subtask_tests) in all_tests.iter().enumerate() {
            for test in subtask_tests {
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
        }

        Ok(())
    }

    /// Records what every generated test is made of.
    ///
    /// The file names come from the task's own naming closures even in seed mode,
    /// where nothing is written: a judge that later materialises the tests should
    /// get the same names a normal run would have produced.
    fn build_manifest(&self, seed: u64, all_tests: &[Vec<GeneratedTest>]) -> Result<Manifest> {
        let mut subtasks = Vec::new();
        let mut global_test_id = 0_i32;
        // The names come from user supplied closures, and two tests that map to
        // the same name would overwrite each other on disk and collide in the
        // archive. Catching it here means seed mode, which writes no files at all,
        // rejects the same task a normal run would.
        let mut used_names: HashSet<String> = HashSet::new();

        for (subtask_idx, subtask_tests) in all_tests.iter().enumerate() {
            let mut tests = Vec::new();
            for (test_id_in_subtask, test) in subtask_tests.iter().enumerate() {
                let input_file = (self.get_input_file_name)(global_test_id, subtask_idx as i32, test_id_in_subtask as i32);
                let output_file = (self.get_output_file_name)(global_test_id, subtask_idx as i32, test_id_in_subtask as i32);

                for name in [&input_file, &output_file] {
                    if !used_names.insert(name.clone()) {
                        return Err(Error::TestAlreadyExists { path: name.clone() });
                    }
                }

                tests.push(ManifestTest {
                    index_in_subtask: test_id_in_subtask,
                    global_index: global_test_id as usize,
                    generator: test.generator,
                    seed: test.seed,
                    input_file,
                    output_file,
                    input_hash: stable_hash(&test.input),
                    output_hash: stable_hash(&test.output),
                });
                global_test_id += 1;
            }

            subtasks.push(ManifestSubtask {
                index: subtask_idx,
                points: self.subtasks[subtask_idx].points,
                name: self.subtasks[subtask_idx].name.clone(),
                tests,
            });
        }

        Ok(Manifest {
            task: self.name.clone(),
            seed,
            trim_whitespace: self.trim_whitespace,
            time_limit: self.time_limit,
            subtasks,
        })
    }

    /// Writes every generated test to the file names the manifest gave it.
    fn write_tests(&self, manifest: &Manifest, all_tests: &[Vec<GeneratedTest>]) -> Result<()> {
        for (subtask, subtask_tests) in manifest.subtasks.iter().zip(all_tests) {
            for (entry, test) in subtask.tests.iter().zip(subtask_tests) {
                let input_path = self.tests_path.join(&entry.input_file);
                let output_path = self.tests_path.join(&entry.output_file);

                fs::write(&input_path, test.input.as_bytes()).map_err(|err| Error::IOError { err, file: path_str(&input_path) })?;
                fs::write(&output_path, test.output.as_bytes()).map_err(|err| Error::IOError { err, file: path_str(&output_path) })?;
            }
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
    fn archive_tests(&self, manifest: &Manifest) -> Result<()> {
        let mut test_files_vec = Vec::new();
        for subtask in &manifest.subtasks {
            for test in &subtask.tests {
                test_files_vec.push(self.tests_path.join(&test.input_file));
                test_files_vec.push(self.tests_path.join(&test.output_file));
            }
        }

        archive_files(&test_files_vec, &self.tests_archive_path, &self.logger)?;

        Ok(())
    }
}
