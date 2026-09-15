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

/// What one candidate test did to the solutions it was run past.
struct CandidateOutcome {
    /// The official solution's output, normalised as it would be written out.
    output: String,
    /// Indices into `Task::solutions` of the solutions that were handed over as
    /// "bad" and did fail this test.
    failed: Vec<usize>,
}

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
                bad_solution_handles.push((i, solution_handles[i]));
            }
        }

        let mut tried_inputs = HashSet::new();
        let mut subtask_tests = Vec::new();
        // How many of the tests kept so far each solution has failed, indexed the
        // same way as `self.solutions`. Only the entries of solutions that are
        // meant to fail this subtask are ever looked at.
        let mut failures = vec![0_usize; self.solutions.len()];

        let total_initial: usize = subtask.initial_counts.iter().sum();
        let target_failures = if bad_solution_handles.is_empty() {
            0
        } else {
            subtask.min_failures_per_solution.unwrap_or(self.min_failures_per_solution)
        };

        // The solutions that still owe failures, which are the only ones worth
        // running: one that has already failed often enough can no longer change
        // what happens to a candidate, and a solution that is meant to fail is
        // usually the slowest thing in the batch, because failing often means
        // running into the time limit.
        let still_owed = |failures: &[usize]| -> Vec<(usize, ProgramHandle)> { bad_solution_handles.iter().copied().filter(|&(sol_idx, _)| failures[sol_idx] < target_failures).collect() };

        let found_count_progress_bar = ScopedProgressBar::new(&self.logger, (total_initial + target_failures) as u64);
        let tries_progress_bar = ScopedProgressBar::new(&self.logger, self.max_tries as u64);

        // Phase 1 (optional): Stress tests
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

        // Phase 2: initial tests from each generator.
        //
        // The solutions that are meant to fail the subtask run here too, even
        // though the test is kept whatever they do: these tests are part of the
        // finished subtask, so one of them breaking a solution is a failure that
        // counts, and phase 3 has that much less left to look for.
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

        // Phase 3: supplemental tests, until every solution that is meant to fail
        // this subtask has failed at least `target_failures` of its tests.
        //
        // The goal is counted per solution rather than by tests that defeat all
        // of them at once: with several bad solutions, the chance that one
        // candidate happens to break every single one of them falls off a cliff,
        // and requiring it turns a search that converges in a handful of tries
        // into one that runs out of tries having found nothing.
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
            // A candidate earns its place by breaking at least one solution that
            // still owes failures. Keeping it for anything less would pad the
            // subtask with tests that separate nothing.
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
            // How many extra tests it takes is not known in advance - one of them
            // can break every solution at once, or only ever one - so the bar
            // grows rather than sitting at full while the search continues.
            let found = found_count_progress_bar.position();
            if found > found_count_progress_bar.length().unwrap_or(0) {
                found_count_progress_bar.set_length(found);
            }
            tries_progress_bar.reset();
        }

        self.report_failures(subtask_idx, subtask, &bad_solution_handles, &failures, target_failures, !subtask_tests.is_empty())?;

        // Shuffle all tests for this subtask, from the run's own generator so that
        // the order is part of what a seed reproduces.
        rng.shuffle(&mut subtask_tests);

        Ok(subtask_tests)
    }

    /// Reports how the finished subtask did against the solutions that are meant
    /// to fail it.
    ///
    /// A solution that never failed a single test of the subtask will pass it
    /// when the finished tests are judged, and that is already a failed run.
    /// Saying so here, rather than after every remaining subtask has been
    /// generated and every solution judged, is the difference between a report
    /// in seconds and one in hours. Falling short of the target while still
    /// failing something only weakens the test data, so that is a complaint
    /// rather than an error.
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

    /// Runs one candidate test past the official solution, the solutions that are
    /// meant to pass this subtask, and a set of solutions that are meant to fail
    /// it.
    ///
    /// The official solution and the good ones have to agree: a test whose answer
    /// is not well defined is an error rather than a test. What the bad solutions
    /// do is only reported, because a test is worth keeping whether or not any
    /// particular one of them happens to fall over on it - the caller decides
    /// what the failures it reports are worth.
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

        // Report which of the bad solutions this test breaks. A wrong answer, a
        // timeout and a crash all count as failing it.
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
