use crate::progress::ScopedProgressBar;
use crate::rng::Rng;
use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use crate::stub::stable_hash;
use crate::{Error, Result, Subtask, Task, ToOutput};
use log::{error, info, warn};
use std::collections::HashSet;
use std::sync::Arc;

/// Turns each run of whitespace into one newline if it contains one and one
/// space otherwise, drops leading whitespace and ends with a single newline.
pub fn trim_whitespace(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for line in input.lines() {
        let mut words = line.split_whitespace();
        if let Some(first) = words.next() {
            result.push_str(first);
            for word in words {
                result.push(' ');
                result.push_str(word);
            }
            result.push('\n');
        }
    }
    if result.is_empty() {
        result.push('\n');
    }
    result
}

/// How many duplicate tests a generator may produce in its initial batch before
/// it is given up on, since one with a small range may never reach its count.
const MAX_REPEATED_TESTS: usize = 100;

/// A kept test. `generator` and `seed` rebuild `input`, which is what a stub
/// records.
pub struct GeneratedTest {
    pub generator: usize,
    pub seed: u64,
    pub input: Arc<str>,
    pub output: Arc<str>,
}

/// A solution's index into `Task::solutions`, and its program.
type IndexedProgram = (usize, ProgramHandle);

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
        let (good, bad): (Vec<IndexedProgram>, Vec<IndexedProgram>) = solution_handles.iter().copied().enumerate().partition(|&(i, _)| !self.solutions[i].should_fail(subtask_idx));
        let target_failures = if bad.is_empty() {
            0
        } else {
            subtask.min_failures_per_solution.unwrap_or(self.min_failures_per_solution)
        };

        let mut tried_inputs = HashSet::new();
        let mut subtask_tests = Vec::new();
        // Indexed like `self.solutions`.
        let mut failures = vec![0_usize; self.solutions.len()];
        // Only these are run: the others can no longer change anything, and bad
        // solutions are the slow ones, since they often run into the time limit.
        let still_owed = |failures: &[usize]| -> Vec<IndexedProgram> { bad.iter().copied().filter(|&(i, _)| failures[i] < target_failures).collect() };

        let total_initial: usize = subtask.initial_counts.iter().sum();
        let found_progress_bar = ScopedProgressBar::new(&self.logger, (total_initial + target_failures) as u64);
        let tries_progress_bar = ScopedProgressBar::new(&self.logger, self.max_tries as u64);

        for gen_idx in (0..subtask.get_num_generators()).filter(|_| subtask.stress_tests > 0) {
            info!("Stress testing generator {gen_idx}");
            let stress_progress_bar = ScopedProgressBar::new(&self.logger, subtask.stress_tests as u64);
            for _ in 0..subtask.stress_tests {
                let input = self.generate_input(subtask_idx, gen_idx, rng.next_seed());
                stress_progress_bar.inc(1);
                self.run_candidate(&input, solution_handle, &good, &[], cpp_runner, subtask_idx, gen_idx)?;
            }
        }

        // The initial tests are kept whatever the bad solutions do, but failures
        // on them still count towards the target.
        for (gen_idx, &needed) in subtask.initial_counts.iter().enumerate() {
            let (mut got, mut repeats) = (0, 0);
            while got < needed && repeats < MAX_REPEATED_TESTS {
                let seed = rng.next_seed();
                let input = self.generate_input(subtask_idx, gen_idx, seed);
                if !tried_inputs.insert(stable_hash(&input)) {
                    repeats += 1;
                    continue;
                }

                let (output, failed) = self.run_candidate(&input, solution_handle, &good, &still_owed(&failures), cpp_runner, subtask_idx, gen_idx)?;
                for i in failed {
                    failures[i] += 1;
                }
                subtask_tests.push(GeneratedTest {
                    generator: gen_idx,
                    seed,
                    input: input.into(),
                    output: output.into(),
                });
                found_progress_bar.inc(1);
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
        let mut tries = 0;
        while !still_owed(&failures).is_empty() && tries < self.max_tries {
            tries += 1;
            tries_progress_bar.inc(1);
            let Some(gen_idx) = subtask.pick_generator(rng) else { break };
            let seed = rng.next_seed();
            let input = self.generate_input(subtask_idx, gen_idx, seed);
            if !tried_inputs.insert(stable_hash(&input)) {
                continue;
            }

            let (output, failed) = self.run_candidate(&input, solution_handle, &good, &still_owed(&failures), cpp_runner, subtask_idx, gen_idx)?;
            if failed.is_empty() {
                continue;
            }
            for i in failed {
                failures[i] += 1;
            }
            subtask_tests.push(GeneratedTest {
                generator: gen_idx,
                seed,
                input: input.into(),
                output: output.into(),
            });
            tries = 0;
            tries_progress_bar.reset();
            found_progress_bar.inc(1);
            // The number of extra tests is not known in advance.
            found_progress_bar.set_length(found_progress_bar.length().unwrap_or(0).max(found_progress_bar.position()));
        }

        if target_failures > 0 && !subtask_tests.is_empty() {
            self.report_failures(subtask_idx, subtask, &bad, &failures, target_failures)?;
        }

        // With the run's rng, so the order is reproducible too.
        rng.shuffle(&mut subtask_tests);
        Ok(subtask_tests)
    }

    /// A bad solution with no failures at all will pass the subtask, so that is
    /// reported now rather than after all the remaining subtasks. Falling short
    /// of the target only weakens the tests, so that is just logged.
    fn report_failures(&self, subtask_idx: usize, subtask: &Subtask<T>, bad: &[IndexedProgram], failures: &[usize], target_failures: usize) -> Result<()> {
        if let Some(&(i, _)) = bad.iter().find(|&&(i, _)| failures[i] == 0) {
            return Err(Error::PartialSolutionPassesExtraSubtask {
                subtask_number: subtask_idx + 1,
                partial_number: i + 1,
                partial_name: self.solutions[i].name.clone(),
                subtask_name: subtask.name.clone(),
            });
        }

        for &(i, _) in bad.iter().filter(|&&(i, _)| failures[i] < target_failures) {
            error!(
                "Subtask {} ({}) only has {} of the {} tests that partial solution {} ({}) is supposed to fail.",
                subtask_idx + 1,
                subtask.name,
                failures[i],
                target_failures,
                i + 1,
                self.solutions[i].name
            );
        }
        Ok(())
    }

    fn write_problem_file(&self, name: &str, contents: &str) -> Result<()> {
        let path = self.problem_path.join(name);
        std::fs::write(&path, contents).map_err(Error::io(&path))
    }

    /// Runs a candidate through the official solution and the given good and bad
    /// solutions. A good solution failing is an error. Returns the official
    /// output and the bad solutions that failed.
    fn run_candidate(
        &self,
        input: &str,
        main_prog: ProgramHandle,
        good: &[IndexedProgram],
        bad: &[IndexedProgram],
        runner: &mut CppRunner,
        subtask_idx: usize,
        gen_idx: usize,
    ) -> Result<(String, Vec<usize>)> {
        let programs: Vec<_> = std::iter::once(main_prog).chain(good.iter().chain(bad).map(|&(_, handle)| handle)).collect();
        let results = runner.check_programs(input, &programs, self.time_limit)?;

        let official_output = match results[0].official_output("generation phase", gen_idx + 1) {
            Ok(output) => self.normalise_output(output),
            Err(err) => return self.write_problem_file("failing_test.in", input).and(Err(err)),
        };
        if !(self.checker)(input, &official_output, &official_output) {
            self.write_problem_file("failing_test.in", input)?;
            return Err(Error::SolutionFailed {
                test_path: "generation phase".to_owned(),
                gen_id: gen_idx + 1,
            });
        }
        let accepted = |result: &RunResult| matches!(result, RunResult::Ok(_, output) if (self.checker)(input, &official_output, output));

        for (&(i, _), result) in good.iter().zip(&results[1..]) {
            if accepted(result) {
                continue;
            }
            self.write_problem_file("failing_test.in", input)?;
            self.write_problem_file("failing_test_correct_output.out", &official_output)?;
            let wrong_output = self.problem_path.join("failing_test_wrong_output.out");
            match result {
                RunResult::Ok(_, output) => self.write_problem_file("failing_test_wrong_output.out", output)?,
                _ if wrong_output.is_file() => std::fs::remove_file(&wrong_output).map_err(Error::io(&wrong_output))?,
                _ => {}
            }
            return Err(Error::PartialSolutionFailsSubtask {
                partial_number: i + 1,
                subtask_number: subtask_idx + 1,
                subtask_name: self.subtasks[subtask_idx].name.clone(),
                partial_name: self.solutions[i].name.clone(),
                verdict: match result {
                    RunResult::Ok(..) => "WA",
                    RunResult::TimedOut => "TLE",
                    RunResult::Crashed => "RTE",
                }
                .to_owned(),
                gen_id: gen_idx + 1,
            });
        }

        let failed = bad.iter().zip(&results[1 + good.len()..]).filter(|(_, result)| !accepted(result)).map(|(&(i, _), _)| i).collect();
        Ok((official_output, failed))
    }
}
