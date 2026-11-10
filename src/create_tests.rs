//! A whole run: generating every subtask's tests, verifying them against the
//! solutions and writing them out.

use crate::archiver::archive_files;
use crate::generate::GeneratedTest;
use crate::mode::Mode;
use crate::parallel::parallel_map;
use crate::progress::ScopedProgressBar;
use crate::rng::Rng;
use crate::runner::cpp_runner::CppRunner;
use crate::stub::{Part, Stub, stable_hash};
use crate::{Error, Result, Task, ToOutput};
use console::style;
use log::{debug, info, warn};
use std::borrow::Cow;
use std::collections::HashSet;
use std::fs;
use std::panic::resume_unwind;
use std::path::Path;

/// The name and contents of a test file.
type TestFile<'test> = (String, Cow<'test, str>);

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

/// Every test, in order, with the index of its subtask.
fn with_subtasks(all_tests: &[Vec<GeneratedTest>]) -> impl Iterator<Item = (usize, &GeneratedTest)> {
    all_tests.iter().enumerate().flat_map(|(subtask_idx, tests)| tests.iter().map(move |test| (subtask_idx, test)))
}

impl<T: ToOutput> Task<T> {
    pub(crate) fn create_tests(&self, mode: Mode, seed: u64) -> Result<()> {
        self.logger.println("").ok();
        self.print_title(&format!("Creating tests for task \"{}\"", self.name));

        if self.subtasks.is_empty() {
            warn!("No subtasks defined.");
        }
        self.check_declared_subtasks_exist()?;
        if self.solution_source.is_empty() {
            return Err(Error::MissingSolution);
        }

        let mut runner = CppRunner::new(&self.build_folder_path)?;
        if let Some(jobs) = self.jobs {
            runner.set_workers(jobs);
        }
        let official = runner.add_program(&self.solution_source)?;
        let partials = self.solutions.iter().map(|solution| runner.add_program(&solution.source)).collect::<Result<Vec<_>>>()?;
        // Only after every program has been added: anything else is stale.
        runner.clean_build_folder()?;

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
            all_tests.push(self.create_tests_for_subtask(subtask_idx, &mut rng, &runner, official, &partials)?);
        }

        self.check_tests_are_reproducible(self.reproducibility_checks(mode), &all_tests, runner.workers())?;
        let names = self.assign_file_names(&all_tests)?;

        self.log_result("Running official solution:")?;
        let lines_of_code = |source: &str| source.split('\n').count();
        let passed_subtasks = self.run_solution_on_tests(&all_tests, &runner, official, lines_of_code(&self.solution_source), &|_subtask_idx| false)?;
        self.warn_about_unreproduced_passes("The official solution", |_subtask_idx| true, &passed_subtasks, &all_tests);

        for (i, (solution, &program)) in self.solutions.iter().zip(&partials).enumerate() {
            self.log_result(&format!("Running partial solution {}: {}", i + 1, solution.name))?;
            let passed_subtasks = self.run_solution_on_tests(&all_tests, &runner, program, lines_of_code(&solution.source), &|subtask_idx| solution.should_fail(subtask_idx))?;
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
        let files = Self::test_files(mode, names, &all_tests);
        for (name, contents) in &files {
            let path = self.tests_path.join(name);
            fs::write(&path, contents.as_bytes()).map_err(Error::io(&path))?;
        }
        archive_files(&files, &self.tests_archive_path, runner.workers(), &self.logger)?;

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
    fn check_tests_are_reproducible(&self, times: usize, all_tests: &[Vec<GeneratedTest>], workers: usize) -> Result<()> {
        let tests: Vec<_> = with_subtasks(all_tests).collect();
        if times == 0 || tests.is_empty() {
            return Ok(());
        }

        info!("Checking that all {} tests can be rebuilt from their seeds ({times} times each)", tests.len());
        let progress_bar = ScopedProgressBar::new(&self.logger, (tests.len() * times) as u64);
        let (subtasks, trim_whitespace) = (&self.subtasks, self.trim_whitespace);
        // For each test, its first rebuild that differs from it.
        let mismatches = parallel_map(&tests, workers, |&(subtask_idx, test)| {
            (1..=times).find_map(|attempt| {
                let rebuilt = subtasks[subtask_idx].generate_input(test.generator, test.seed, trim_whitespace);
                progress_bar.inc(1);
                (rebuilt != test.input).then_some((attempt, rebuilt))
            })
        });

        for (&(subtask_idx, test), mismatch) in tests.iter().zip(mismatches) {
            match mismatch {
                Ok(None) => {}
                Ok(Some((attempt, rebuilt))) => {
                    return Err(Error::GeneratorNotReproducible {
                        subtask_number: subtask_idx + 1,
                        gen_id: test.generator + 1,
                        seed: format!("{:#018x}", test.seed),
                        attempt,
                        attempts: times,
                        details: describe_difference(&test.input, &rebuilt),
                    });
                }
                Err(panic) => resume_unwind(panic),
            }
        }
        Ok(())
    }

    /// The input and output file names of every test, in order.
    fn assign_file_names(&self, all_tests: &[Vec<GeneratedTest>]) -> Result<Vec<(String, String)>> {
        let mut names = Vec::new();
        // The naming closures come from the user and may map two tests to one name.
        let mut used_names = HashSet::new();
        for (subtask_idx, subtask_tests) in all_tests.iter().enumerate() {
            for test_in_subtask in 0..subtask_tests.len() {
                let files = (
                    (self.input_file_name)(names.len(), subtask_idx, test_in_subtask),
                    (self.output_file_name)(names.len(), subtask_idx, test_in_subtask),
                );
                if let Some(name) = [&files.0, &files.1].into_iter().find(|&name| !used_names.insert(name.clone())) {
                    return Err(Error::TestAlreadyExists { path: name.clone() });
                }
                names.push(files);
            }
        }
        Ok(names)
    }

    /// Every test file: each test's data, or in seed mode the stub that
    /// rebuilds it.
    fn test_files(mode: Mode, names: Vec<(String, String)>, all_tests: &[Vec<GeneratedTest>]) -> Vec<TestFile<'_>> {
        let mut files = Vec::with_capacity(2 * names.len());
        for ((input_name, output_name), (subtask, test)) in names.into_iter().zip(with_subtasks(all_tests)) {
            for (name, part, contents) in [(input_name, Part::Input, &test.input), (output_name, Part::Output, &test.output)] {
                let contents = if mode == Mode::Files {
                    Cow::Borrowed(contents.as_str())
                } else {
                    let stub = Stub {
                        subtask,
                        generator: test.generator,
                        seed: test.seed,
                        part,
                        hash: Some(stable_hash(contents)),
                    };
                    Cow::Owned(stub.to_line())
                };
                files.push((name, contents));
            }
        }
        files
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
