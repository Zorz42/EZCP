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
            // Separate from the previous run of whitespace, unless nothing has been
            // written yet: leading whitespace is dropped rather than turned into an
            // indent or a blank first line.
            if in_block && !result.is_empty() {
                result.push(if has_newline { '\n' } else { ' ' });
            }
            in_block = false;
            has_newline = false;
            result.push(c);
        }
    }

    // ensure trailing newline
    if !result.ends_with('\n') {
        result.push('\n');
    }

    result
}

/// How many times in a row a generator may repeat a test it has already produced
/// before the initial batch gives up on it.
///
/// A generator with a small range runs out of distinct tests long before it has
/// produced the requested count, and without a bound it would spin forever.
const MAX_REPEATED_TESTS: usize = 100;

/// A finished test, together with the recipe that produced it.
///
/// The recipe is what makes the test disposable: `generator` and `seed` are
/// enough to build `input` again from nothing, which is what a stub records and
/// what the on-demand server replays.
pub struct GeneratedTest {
    /// Which of the subtask's generators produced this test.
    pub generator: usize,
    /// The seed that generator was run with.
    pub seed: u64,
    /// The test input, exactly as it would be written to a `.in` file.
    pub input: Arc<str>,
    /// The official solution's output, exactly as it would be written to a
    /// `.out` file.
    pub output: Arc<str>,
}

impl<T: ToOutput> Task<T> {
    /// Applies to a generated input whatever normalisation the task asked for.
    ///
    /// Both test generation and the on-demand server go through here, which is
    /// what makes a served test byte-for-byte the file a normal run would write.
    pub(crate) fn normalise_input(&self, raw: String) -> String {
        if self.trim_whitespace { trim_whitespace(&raw) } else { raw }
    }

    /// Applies to the official solution's output whatever normalisation the task
    /// asked for.
    pub(crate) fn normalise_output(&self, raw: &str) -> String {
        // The trailing newline is added regardless of `trim_whitespace`: an output
        // file that does not end in one is a nuisance for every judge that reads it.
        let trimmed = raw.trim().to_owned() + "\n";
        if self.trim_whitespace { trim_whitespace(&trimmed) } else { trimmed }
    }

    /// Generates the input that `generator` produces from `seed`.
    ///
    /// This is the single definition of what a (generator, seed) pair means, used
    /// by generation and by the server alike.
    ///
    /// # Panics
    /// Panics if `subtask_idx` or `generator` name something the task does not have.
    pub(crate) fn generate_input(&self, subtask_idx: usize, generator: usize, seed: u64) -> String {
        self.normalise_input(self.subtasks[subtask_idx].generate_test(generator, seed).to_output())
    }

    /// Generates and verifies every test of one subtask.
    ///
    /// Nothing is written to disk here: the tests come back in memory, and what
    /// happens to them depends on the mode the task is running in.
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
                bad_solution_handles.push(solution_handles[i]);
            }
        }

        let mut tried_inputs = HashSet::new();
        let mut subtask_tests = Vec::new();
        let mut robust_found_count = 0;

        let total_initial: usize = subtask.initial_counts.iter().sum();
        let target_robust = if bad_solution_handles.is_empty() {
            0
        } else {
            subtask.min_failures_per_solution.unwrap_or(self.min_failures_per_solution)
        };

        let found_count_progress_bar = ScopedProgressBar::new(&self.logger, (total_initial + target_robust) as u64);
        let tries_progress_bar = ScopedProgressBar::new(&self.logger, self.max_tries as u64);

        // Phase 1 (optional): Stress tests
        if subtask.stress_tests != 0 {
            for gen_idx in 0..subtask.get_num_generators() {
                info!("Stress testing generator {gen_idx}");
                let stress_testing_progress_bar = ScopedProgressBar::new(&self.logger, subtask.stress_tests as u64);
                for _ in 0..subtask.stress_tests {
                    let test_str = self.generate_input(subtask_idx, gen_idx, rng.next_seed());

                    stress_testing_progress_bar.inc(1);

                    self.is_robust_test(&test_str, solution_handle, &good_solution_handles, &[], cpp_runner, subtask_idx, gen_idx)?;
                }
            }
        }

        // Phase 2: Initial tests from each generator (only good solutions must pass)
        for gen_idx in 0..subtask.get_num_generators() {
            let needed = subtask.initial_counts.get(gen_idx).copied().unwrap_or(0);
            let mut got = 0;
            let mut fails = 0;
            while got < needed && fails < MAX_REPEATED_TESTS {
                let seed = rng.next_seed();
                let candidate = self.generate_input(subtask_idx, gen_idx, seed);
                // Each test must be unique within the subtask
                if !tried_inputs.insert(stable_hash(&candidate)) {
                    fails += 1;
                    continue;
                }

                // Only good solutions are checked here (no bad_progs passed)
                let Some(main_output) = self.is_robust_test(&candidate, solution_handle, &good_solution_handles, &[], cpp_runner, subtask_idx, gen_idx)? else {
                    unreachable!("is_robust_test with no bad progs should always return Some or Err")
                };
                subtask_tests.push(GeneratedTest {
                    generator: gen_idx,
                    seed,
                    input: Arc::from(candidate),
                    output: Arc::from(main_output),
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

        // Phase 3: Robust tests (failing bad solutions)
        let mut supplemental_tries = 0;
        while robust_found_count < target_robust && supplemental_tries < self.max_tries {
            supplemental_tries += 1;
            tries_progress_bar.inc(1);
            let Some(gen_idx) = subtask.pick_generator(rng) else { break };
            let seed = rng.next_seed();
            let candidate = self.generate_input(subtask_idx, gen_idx, seed);
            if !tried_inputs.insert(stable_hash(&candidate)) {
                continue;
            }

            if let Some(main_output) = self.is_robust_test(&candidate, solution_handle, &good_solution_handles, &bad_solution_handles, cpp_runner, subtask_idx, gen_idx)? {
                subtask_tests.push(GeneratedTest {
                    generator: gen_idx,
                    seed,
                    input: Arc::from(candidate),
                    output: Arc::from(main_output),
                });
                robust_found_count += 1;
                supplemental_tries = 0;
                found_count_progress_bar.inc(1);
                tries_progress_bar.reset();
            }
        }

        if robust_found_count < target_robust {
            error!("Could not find enough robust tests for Subtask {} (found {}/{})", subtask_idx + 1, robust_found_count, target_robust);
        }

        // Shuffle all tests for this subtask, from the run's own generator so that
        // the order is part of what a seed reproduces.
        rng.shuffle(&mut subtask_tests);

        Ok(subtask_tests)
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
        gen_idx: usize,
    ) -> Result<Option<String>> {
        let mut all_progs = vec![main_prog];
        for &(_, handle) in good_progs {
            all_progs.push(handle);
        }
        all_progs.extend_from_slice(bad_progs);

        // Run all solutions in parallel
        let results = runner.check_programs(input, &all_progs, self.time_limit)?;

        let write_bad_test = || -> Result<()> {
            let write_path = self.problem_path.join("failing_test.in");
            fs::write(write_path.clone(), input).map_err(move |err| Error::IOError { file: path_str(&write_path), err })?;
            Ok(())
        };

        // Correct (Main) Solution Result
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

        // Ensure all other "good" solutions pass and match main output
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

        if bad_progs.is_empty() {
            return Ok(Some(correct_output));
        }

        // Run Bad Solutions to ensure they fail
        let bad_results_start = 1 + good_progs.len();
        for res in &results[bad_results_start..] {
            match res {
                RunResult::Ok(_, output) if (self.checker)(input, &correct_output, output) => {
                    // A bad solution passed this test! This test is not robust enough.
                    return Ok(None);
                }
                _ => {} // Bad solution failed as expected (TLE, Crash, or WA)
            }
        }
        Ok(Some(correct_output))
    }
}
