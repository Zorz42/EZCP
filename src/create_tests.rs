use crate::archiver::archive_files;
use crate::generate::GeneratedTest;
use crate::mode::Mode;
use crate::parallel::parallel_map;
use crate::progress::ScopedProgressBar;
use crate::report::{Mismatch, PartialReport, Report, SubtaskInfo, SubtaskResult, TestInfo};
use crate::rng::Rng;
use crate::stub::{Part, Stub, stable_hash};
use crate::{Error, Result, Task, ToOutput};
use console::style;
use log::{debug, info, warn};
use std::borrow::Cow;
use std::collections::HashSet;
use std::fs;
use std::panic::resume_unwind;
use std::path::Path;

type TestFile<'test> = (String, Cow<'test, str>);

fn format_size(bytes: usize) -> String {
    let bytes = bytes as f32;
    if bytes < 1_000_000.0 {
        format!("{:.1}kB", bytes / 1_000.0)
    } else {
        format!("{:.2}MB", bytes / 1_000_000.0)
    }
}

/// On Windows a virus scanner or the indexer often holds an old test file open for a moment.
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

fn describe_difference(first: &str, second: &str) -> String {
    const EXCERPT_WIDTH: usize = 24;

    fn excerpt(text: &str, offset: usize) -> String {
        let bytes = text.as_bytes();
        let end = offset.saturating_add(EXCERPT_WIDTH).min(bytes.len());
        // The cut may split a character.
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

fn with_subtasks(all_tests: &[Vec<GeneratedTest>]) -> impl Iterator<Item = (usize, &GeneratedTest)> {
    all_tests.iter().enumerate().flat_map(|(subtask_idx, tests)| tests.iter().map(move |test| (subtask_idx, test)))
}

impl<T: ToOutput> Task<T> {
    /// Fills in `report` as it goes.
    pub(crate) fn create_tests(&self, mode: Mode, seed: u64, report: &mut Report) -> Result<()> {
        self.logger.println("").ok();
        self.print_title(&format!("Creating tests for task \"{}\"", self.name));

        report.seed = Some(seed);
        report.subtasks = self
            .subtasks
            .iter()
            .map(|subtask| SubtaskInfo {
                name: subtask.name.clone(),
                points: subtask.points,
            })
            .collect();
        report.partials = self
            .solutions
            .iter()
            .map(|solution| {
                let mut declared: Vec<_> = solution.passes_subtasks.iter().copied().collect();
                declared.sort_unstable();
                PartialReport {
                    name: solution.name.clone(),
                    declared,
                    results: Vec::new(),
                }
            })
            .collect();

        if self.subtasks.is_empty() {
            warn!("No subtasks defined.");
        }
        self.check_declared_subtasks_exist()?;
        if self.solution_source.is_empty() {
            return Err(Error::MissingSolution);
        }

        let runner = cpprunner::Runner::new(&self.build_folder_path)?;
        runner.set_workers(self.workers());
        runner.set_result_cache(self.outcome_cache);
        runner.set_timing_retries(self.timing_retries);
        let official = self.solution_source.as_str();
        let partials: Vec<&str> = self.solutions.iter().map(|solution| solution.source.as_str()).collect();

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
            all_tests.push(self.create_tests_for_subtask(subtask_idx, &mut rng, &runner, official, &partials, &mut report.mismatches)?);
        }

        self.check_tests_are_reproducible(self.reproducibility_checks(mode), &all_tests, self.workers())?;
        let names = self.assign_file_names(&all_tests)?;
        report.tests = names
            .iter()
            .zip(with_subtasks(&all_tests))
            .map(|((input_file, output_file), (subtask, test))| TestInfo {
                subtask,
                generator: test.generator,
                seed: test.seed,
                input_file: input_file.clone(),
                output_file: output_file.clone(),
            })
            .collect();

        self.log_result("Running official solution:")?;
        let lines_of_code = |source: &str| source.split('\n').count();
        report.official = self.run_solution_on_tests(&all_tests, &runner, official, lines_of_code(&self.solution_source), &|_subtask_idx| false)?;
        self.warn_about_unreproduced_passes("The official solution", |_subtask_idx| true, &report.official, &all_tests);

        self.run_partial_solutions(&all_tests, &runner, &partials, report)?;
        if !report.mismatches.is_empty() {
            return Err(Error::DeclarationsMismatch { count: report.mismatches.len() });
        }

        // Only now, so a failed run leaves no tests.
        let files = Self::test_files(mode, names, &all_tests);
        for (name, contents) in &files {
            let path = self.tests_path.join(name);
            fs::write(&path, contents.as_bytes()).map_err(Error::io(&path))?;
        }
        archive_files(&files, &self.tests_archive_path, self.workers(), &self.logger)?;

        let size = files.iter().map(|(_, contents)| contents.len()).sum();
        self.log_result(&format!("Tests size: {}", style(format_size(size)).bold()))?;
        if mode == Mode::Seeds {
            self.log_result("The test files are seeds: pipe one into the task with --serve to rebuild it")?;
        }
        for (i, tests) in all_tests.iter().enumerate() {
            self.log_result(&format!("Subtask {}: {} tests", i + 1, tests.len()))?;
        }
        Ok(())
    }

    /// Runs each partial solution on the finished tests. Generation only finds one passing a subtask
    /// it should fail when it runs out of tries, so this records the rest.
    fn run_partial_solutions(&self, all_tests: &[Vec<GeneratedTest>], runner: &cpprunner::Runner, partials: &[&str], report: &mut Report) -> Result<()> {
        let lines_of_code = |source: &str| source.split('\n').count();
        for (i, (solution, &program)) in self.solutions.iter().zip(partials).enumerate() {
            self.log_result(&format!("Running partial solution {}: {}", i + 1, solution.name))?;
            let results = self.run_solution_on_tests(all_tests, runner, program, lines_of_code(&solution.source), &|subtask_idx| solution.should_fail(subtask_idx))?;
            for subtask_idx in (0..self.subtasks.len()).filter(|&idx| results[idx].passed() && solution.should_fail(idx)) {
                let recorded = |mismatch: &Mismatch| matches!(*mismatch, Mismatch::Passes { solution, subtask } if solution == i && subtask == subtask_idx);
                if !report.mismatches.iter().any(recorded) {
                    report.mismatches.push(Mismatch::Passes { solution: i, subtask: subtask_idx });
                }
                if !self.keep_going {
                    return Err(self.passes_extra_subtask(i, subtask_idx));
                }
            }
            let failed_already = |idx: usize| {
                report
                    .mismatches
                    .iter()
                    .any(|mismatch| matches!(*mismatch, Mismatch::Fails { solution, subtask, .. } if solution == i && subtask == idx))
            };
            self.warn_about_unreproduced_passes(
                &format!("Partial solution {} ({})", i + 1, solution.name),
                |idx| !solution.should_fail(idx) && !failed_already(idx),
                &results,
                all_tests,
            );
            report.partials[i].results = results;
        }
        Ok(())
    }

    /// Nothing stops a generator from using randomness other than its `Rng`; this catches one before
    /// its seeds are written down.
    fn check_tests_are_reproducible(&self, times: usize, all_tests: &[Vec<GeneratedTest>], workers: usize) -> Result<()> {
        let tests: Vec<_> = with_subtasks(all_tests).collect();
        if times == 0 || tests.is_empty() {
            return Ok(());
        }

        info!("Checking that all {} tests can be rebuilt from their seeds ({times} times each)", tests.len());
        let progress_bar = ScopedProgressBar::new(&self.logger, (tests.len() * times) as u64);
        let (subtasks, trim_whitespace) = (&self.subtasks, self.trim_whitespace);
        let mismatches = parallel_map(&tests, workers, |&(subtask_idx, test)| {
            (1..=times).find_map(|attempt| {
                // A generator that panics on a seed it built a test from is not reproducible either.
                let rebuilt = subtasks[subtask_idx]
                    .generate_input(test.generator, test.seed, trim_whitespace)
                    .unwrap_or_else(|fault| format!("{fault:?}"));
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

    fn assign_file_names(&self, all_tests: &[Vec<GeneratedTest>]) -> Result<Vec<(String, String)>> {
        let mut names = Vec::new();
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

    /// Otherwise a wrong (usually 1-based) index silently makes a solution one that fails everywhere.
    fn check_declared_subtasks_exist(&self) -> Result<()> {
        for (partial_idx, solution) in self.solutions.iter().enumerate() {
            // The smallest, so the error does not depend on hash order.
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

    /// Every test already passed these solutions during generation, so this means flaky verdicts,
    /// usually from a solution right at the time limit.
    fn warn_about_unreproduced_passes(&self, solution: &str, meant_to_pass: impl Fn(usize) -> bool, results: &[SubtaskResult], all_tests: &[Vec<GeneratedTest>]) {
        for (subtask_idx, subtask) in self.subtasks.iter().enumerate() {
            if meant_to_pass(subtask_idx) && !results[subtask_idx].passed() && !all_tests[subtask_idx].is_empty() {
                warn!("{solution} did not pass subtask {} ({}) when it was run on the finished tests.", subtask_idx + 1, subtask.name);
            }
        }
    }
}
