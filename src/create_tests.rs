use crate::Error::SolutionFailed;
use crate::Result;
use crate::progress::ScopedProgressBar;
use crate::rng::Rng;
use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use crate::stub::stable_hash;
use crate::task::path_str;
use crate::{Error, Subtask, Task, ToOutput};
use log::{error, info, warn};
use std::collections::HashSet;
use std::fs;
use std::sync::Arc;

/// Turns each run of whitespace into one newline if it contains one and one
/// space otherwise, drops leading whitespace and ends with a single newline.
pub fn trim_whitespace(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_block = false;
    let mut has_newline = false;

    for c in input.chars() {
        if c.is_whitespace() {
            in_block = true;
            if c == '\n' {
                has_newline = true;
            }
        } else {
            if in_block && !result.is_empty() {
                result.push(if has_newline { '\n' } else { ' ' });
            }
            in_block = false;
            has_newline = false;
            result.push(c);
        }
    }

    if !result.ends_with('\n') {
        result.push('\n');
    }

    result
}

/// How many duplicate tests a generator may produce in its initial batch before
/// it is given up on, since one with a small range may never reach its count.
const MAX_REPEATED_TESTS: usize = 100;

struct CandidateOutcome {
    output: String,
    /// Indices into `Task::solutions` of the bad solutions that failed.
    failed: Vec<usize>,
}

/// A kept test. `generator` and `seed` rebuild `input`, which is what a stub
/// records.
pub struct GeneratedTest {
    pub generator: usize,
    pub seed: u64,
    pub input: Arc<str>,
    pub output: Arc<str>,
}

impl<T: ToOutput> Task<T> {
    /// Shared by generation and serving, so a served test matches the file
    /// byte for byte.
    pub(crate) fn normalise_input(&self, raw: String) -> String {
        if self.trim_whitespace { trim_whitespace(&raw) } else { raw }
    }

    pub(crate) fn normalise_output(&self, raw: &str) -> String {
        let trimmed = raw.trim().to_owned() + "\n";
        if self.trim_whitespace { trim_whitespace(&trimmed) } else { trimmed }
    }

    /// # Panics
    /// Panics if `subtask_idx` or `generator` do not exist.
    pub(crate) fn generate_input(&self, subtask_idx: usize, generator: usize, seed: u64) -> String {
        self.normalise_input(self.subtasks[subtask_idx].generate_test(generator, seed).to_output())
    }

    pub(super) fn create_tests_for_subtask(
        &self,
        subtask_idx: usize,
        subtask: &Subtask<T>,
        rng: &mut Rng,
        solution_handles: &[ProgramHandle],
        solution_handle: ProgramHandle,
        cpp_runner: &mut CppRunner,
    ) -> Result<Vec<GeneratedTest>> {
        let mut good_solution_handles = Vec::new();
        let mut bad_solution_handles = Vec::new();
        for (i, solution) in self.solutions.iter().enumerate() {
            if solution.passes_subtasks.contains(&subtask_idx) {
                good_solution_handles.push((i, solution_handles[i]));
            } else {
                bad_solution_handles.push((i, solution_handles[i]));
            }
        }

        let mut tried_inputs = HashSet::new();
        let mut subtask_tests = Vec::new();
        // Indexed like `self.solutions`.
        let mut failures = vec![0_usize; self.solutions.len()];

        let total_initial: usize = subtask.initial_counts.iter().sum();
        let target_failures = if bad_solution_handles.is_empty() {
            0
        } else {
            subtask.min_failures_per_solution.unwrap_or(self.min_failures_per_solution)
        };

        // Only these are run: the others can no longer change anything, and bad
        // solutions are the slow ones, since they often run into the time limit.
        let still_owed = |failures: &[usize]| -> Vec<(usize, ProgramHandle)> { bad_solution_handles.iter().copied().filter(|&(sol_idx, _)| failures[sol_idx] < target_failures).collect() };

        let found_count_progress_bar = ScopedProgressBar::new(&self.logger, (total_initial + target_failures) as u64);
        let tries_progress_bar = ScopedProgressBar::new(&self.logger, self.max_tries as u64);

        if subtask.stress_tests != 0 {
            for gen_idx in 0..subtask.get_num_generators() {
                info!("Stress testing generator {gen_idx}");
                let stress_testing_progress_bar = ScopedProgressBar::new(&self.logger, subtask.stress_tests as u64);
                for _ in 0..subtask.stress_tests {
                    let test_str = self.generate_input(subtask_idx, gen_idx, rng.next_seed());

                    stress_testing_progress_bar.inc(1);

                    self.run_candidate(&test_str, solution_handle, &good_solution_handles, &[], cpp_runner, subtask_idx, gen_idx)?;
                }
            }
        }

        // The initial tests are kept whatever the bad solutions do, but failures
        // on them still count towards the target.
        for gen_idx in 0..subtask.get_num_generators() {
            let needed = subtask.initial_counts.get(gen_idx).copied().unwrap_or(0);
            let mut got = 0;
            let mut fails = 0;
            while got < needed && fails < MAX_REPEATED_TESTS {
                let seed = rng.next_seed();
                let candidate = self.generate_input(subtask_idx, gen_idx, seed);
                if !tried_inputs.insert(stable_hash(&candidate)) {
                    fails += 1;
                    continue;
                }

                let outcome = self.run_candidate(&candidate, solution_handle, &good_solution_handles, &still_owed(&failures), cpp_runner, subtask_idx, gen_idx)?;
                for sol_idx in outcome.failed {
                    failures[sol_idx] += 1;
                }
                subtask_tests.push(GeneratedTest {
                    generator: gen_idx,
                    seed,
                    input: Arc::from(candidate),
                    output: Arc::from(outcome.output),
                });
                found_count_progress_bar.inc(1);
                got += 1;
            }
            if got < needed {
                warn!(
                    "Generator {gen_idx} of subtask {} produced only {got} of {needed} tests, because it kept repeating tests it had already generated.",
                    subtask_idx + 1
                );
            }
        }

        // Supplemental tests, kept only if they break a solution that still owes
        // failures. The target is per solution: requiring one test to break all
        // of them at once rarely succeeds.
        let mut supplemental_tries = 0;
        while !still_owed(&failures).is_empty() && supplemental_tries < self.max_tries {
            supplemental_tries += 1;
            tries_progress_bar.inc(1);
            let Some(gen_idx) = subtask.pick_generator(rng) else { break };
            let seed = rng.next_seed();
            let candidate = self.generate_input(subtask_idx, gen_idx, seed);
            if !tried_inputs.insert(stable_hash(&candidate)) {
                continue;
            }

            let outcome = self.run_candidate(&candidate, solution_handle, &good_solution_handles, &still_owed(&failures), cpp_runner, subtask_idx, gen_idx)?;
            if outcome.failed.is_empty() {
                continue;
            }
            for sol_idx in outcome.failed {
                failures[sol_idx] += 1;
            }
            subtask_tests.push(GeneratedTest {
                generator: gen_idx,
                seed,
                input: Arc::from(candidate),
                output: Arc::from(outcome.output),
            });
            supplemental_tries = 0;
            found_count_progress_bar.inc(1);
            // The number of extra tests is not known in advance.
            let found = found_count_progress_bar.position();
            if found > found_count_progress_bar.length().unwrap_or(0) {
                found_count_progress_bar.set_length(found);
            }
            tries_progress_bar.reset();
        }

        self.report_failures(subtask_idx, subtask, &bad_solution_handles, &failures, target_failures, !subtask_tests.is_empty())?;

        // With the run's rng, so the order is reproducible too.
        rng.shuffle(&mut subtask_tests);

        Ok(subtask_tests)
    }

    /// A bad solution with no failures at all will pass the subtask, so that is
    /// reported now rather than after all the remaining subtasks. Falling short
    /// of the target only weakens the tests, so that is just logged.
    fn report_failures(&self, subtask_idx: usize, subtask: &Subtask<T>, bad_progs: &[(usize, ProgramHandle)], failures: &[usize], target_failures: usize, has_tests: bool) -> Result<()> {
        if target_failures == 0 || !has_tests {
            return Ok(());
        }

        for &(sol_idx, _) in bad_progs {
            if failures[sol_idx] == 0 {
                return Err(Error::PartialSolutionPassesExtraSubtask {
                    subtask_number: subtask_idx + 1,
                    partial_number: sol_idx + 1,
                    partial_name: self.solutions[sol_idx].name.clone(),
                    subtask_name: subtask.name.clone(),
                });
            }
        }

        for &(sol_idx, _) in bad_progs {
            if failures[sol_idx] < target_failures {
                error!(
                    "Subtask {} ({}) only has {} of the {} tests that partial solution {} ({}) is supposed to fail.",
                    subtask_idx + 1,
                    subtask.name,
                    failures[sol_idx],
                    target_failures,
                    sol_idx + 1,
                    self.solutions[sol_idx].name
                );
            }
        }

        Ok(())
    }

    /// Runs a candidate through the official solution and the given good and bad
    /// solutions. A good solution failing is an error; the bad solutions that
    /// failed are returned.
    fn run_candidate(
        &self,
        input: &str,
        main_prog: ProgramHandle,
        good_progs: &[(usize, ProgramHandle)],
        bad_progs: &[(usize, ProgramHandle)],
        runner: &mut CppRunner,
        subtask_idx: usize,
        gen_idx: usize,
    ) -> Result<CandidateOutcome> {
        let mut all_progs = vec![main_prog];
        for &(_, handle) in good_progs {
            all_progs.push(handle);
        }
        for &(_, handle) in bad_progs {
            all_progs.push(handle);
        }

        let results = runner.check_programs(input, &all_progs, self.time_limit)?;

        let write_bad_test = || -> Result<()> {
            let write_path = self.problem_path.join("failing_test.in");
            fs::write(write_path.clone(), input).map_err(move |err| Error::IOError { file: path_str(&write_path), err })?;
            Ok(())
        };

        let correct_output = match &results[0] {
            RunResult::Ok(_, output) => self.normalise_output(output),
            RunResult::TimedOut => {
                write_bad_test()?;
                return Err(Error::SolutionTimedOut {
                    test_path: "generation phase".to_owned(),
                    gen_id: gen_idx + 1,
                });
            }
            RunResult::Crashed => {
                write_bad_test()?;
                return Err(Error::SolutionCrash {
                    test_path: "generation phase".to_owned(),
                    gen_id: gen_idx + 1,
                });
            }
        };

        if !(self.checker)(input, &correct_output, &correct_output) {
            write_bad_test()?;
            return Err(SolutionFailed {
                test_path: "generation phase".to_owned(),
                gen_id: gen_idx + 1,
            });
        }

        for (i, &(sol_idx, _)) in good_progs.iter().enumerate() {
            match &results[i + 1] {
                RunResult::Ok(_, output) if (self.checker)(input, &correct_output, output) => {}
                result => {
                    let write_path = self.problem_path.join("failing_test.in");
                    let official_output_write_path = self.problem_path.join("failing_test_correct_output.out");
                    let wrong_output_write_path = self.problem_path.join("failing_test_wrong_output.out");
                    fs::write(official_output_write_path.clone(), correct_output).map_err(move |err| Error::IOError {
                        file: path_str(&official_output_write_path),
                        err,
                    })?;

                    if let RunResult::Ok(_, output) = &results[i + 1] {
                        fs::write(wrong_output_write_path.clone(), output).map_err(move |err| Error::IOError {
                            file: path_str(&wrong_output_write_path),
                            err,
                        })?;
                    } else if wrong_output_write_path.is_file() {
                        fs::remove_file(wrong_output_write_path.clone()).map_err(move |err| Error::IOError {
                            file: path_str(&wrong_output_write_path),
                            err,
                        })?;
                    }

                    fs::write(write_path.clone(), input).map_err(move |err| Error::IOError { file: path_str(&write_path), err })?;
                    return Err(Error::PartialSolutionFailsSubtask {
                        partial_number: sol_idx + 1,
                        subtask_number: subtask_idx + 1,
                        subtask_name: self.subtasks[subtask_idx].name.clone(),
                        partial_name: self.solutions[sol_idx].name.clone(),
                        verdict: if matches!(result, RunResult::Ok(_, _)) { "WA".to_owned() } else { result.to_display_string() },
                        gen_id: gen_idx + 1,
                    });
                }
            }
        }

        let bad_results_start = 1 + good_progs.len();
        let mut failed = Vec::new();
        for (&(sol_idx, _), result) in bad_progs.iter().zip(&results[bad_results_start..]) {
            let passed = matches!(result, RunResult::Ok(_, output) if (self.checker)(input, &correct_output, output));
            if !passed {
                failed.push(sol_idx);
            }
        }

        Ok(CandidateOutcome { output: correct_output, failed })
    }
}
