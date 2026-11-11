//! Generating one subtask's tests.
//!
//! Candidates are drawn, built and run a wave at a time on several threads, yet the outcome is that of
//! taking them one by one: a wave is judged in order, and where that order says to stop, the rest of
//! it is forgotten and the `Rng` put back to before them. So the tests do not depend on the threads.

use crate::parallel::parallel_map;
use crate::progress::ScopedProgressBar;
use crate::rng::Rng;
use crate::runner::cpp_runner::{CppRunner, Job, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use crate::stub::stable_hash;
use crate::whitespace::trim_whitespace_owned;
use crate::{Error, Result, Subtask, Task, ToOutput};
use log::{error, info, warn};
use std::collections::HashSet;
use std::panic::resume_unwind;

/// Repeats a generator may produce in its initial batch before it is given up on; one with a small
/// range may never reach its count.
const MAX_REPEATED_TESTS: usize = 100;

pub struct GeneratedTest {
    pub generator: usize,
    pub seed: u64,
    pub input: String,
    pub output: String,
}

/// An index into `Task::solutions`, and the program.
type IndexedProgram = (usize, ProgramHandle);

struct Candidate {
    generator: usize,
    seed: u64,
    /// Where the `Rng` goes back to if the candidate's turn never comes.
    rng_before: Rng,
    /// `None` for an input tried before, which was not run. An error is the generator's panic.
    run: std::thread::Result<Option<Run>>,
}

struct Run {
    input: String,
    hash: u64,
    /// Of the official solution, the good solutions and the bad ones that were run, in that order.
    results: Vec<RunResult>,
}

/// A generator's panic is passed on only once its candidate's turn comes, since an earlier candidate
/// may already have ended the run.
fn reached(run: std::thread::Result<Option<Run>>) -> Option<Run> {
    run.unwrap_or_else(|panic| resume_unwind(panic))
}

struct Generation<'task, T: ToOutput> {
    task: &'task Task<T>,
    runner: &'task CppRunner,
    subtask_idx: usize,
    subtask: &'task Subtask<T>,
    official: ProgramHandle,
    /// The partial solutions meant to pass this subtask, and those meant to fail it.
    good: Vec<IndexedProgram>,
    bad: Vec<IndexedProgram>,
    target_failures: usize,
    /// Indexed like `Task::solutions`.
    failures: Vec<usize>,
    tried_inputs: HashSet<u64>,
    tests: Vec<GeneratedTest>,
}

impl<T: ToOutput> Task<T> {
    pub(crate) fn normalise_output(&self, raw: String) -> String {
        if self.trim_whitespace { trim_whitespace_owned(raw) } else { raw.trim().to_owned() + "\n" }
    }

    pub(crate) fn generate_input(&self, subtask_idx: usize, generator: usize, seed: u64) -> String {
        self.subtasks[subtask_idx].generate_input(generator, seed, self.trim_whitespace)
    }

    pub(crate) fn passes_extra_subtask(&self, partial: usize, subtask: usize) -> Error {
        Error::PartialSolutionPassesExtraSubtask {
            subtask_number: subtask + 1,
            partial_number: partial + 1,
            partial_name: self.solutions[partial].name.clone(),
            subtask_name: self.subtasks[subtask].name.clone(),
        }
    }

    pub(crate) fn create_tests_for_subtask(&self, subtask_idx: usize, rng: &mut Rng, runner: &CppRunner, official: ProgramHandle, partials: &[ProgramHandle]) -> Result<Vec<GeneratedTest>> {
        let subtask = &self.subtasks[subtask_idx];
        let (good, bad): (Vec<IndexedProgram>, Vec<IndexedProgram>) = partials.iter().copied().enumerate().partition(|&(i, _)| !self.solutions[i].should_fail(subtask_idx));
        let target_failures = if bad.is_empty() {
            0
        } else {
            subtask.min_failures_per_solution.unwrap_or(self.min_failures_per_solution)
        };
        let mut generation = Generation {
            task: self,
            runner,
            subtask_idx,
            subtask,
            official,
            good,
            bad,
            target_failures,
            failures: vec![0; self.solutions.len()],
            tried_inputs: HashSet::new(),
            tests: Vec::new(),
        };

        let total_initial: usize = subtask.initial_counts.iter().sum();
        let found_progress_bar = ScopedProgressBar::new(&self.logger, (total_initial + target_failures) as u64);
        let tries_progress_bar = ScopedProgressBar::new(&self.logger, self.max_tries as u64);

        generation.stress_test(rng)?;
        generation.generate_initial_tests(rng, &found_progress_bar)?;
        generation.hunt_for_failures(rng, &found_progress_bar, &tries_progress_bar)?;
        if target_failures > 0 && !generation.tests.is_empty() {
            generation.report_failures()?;
        }

        let mut tests = generation.tests;
        rng.shuffle(&mut tests);
        Ok(tests)
    }
}

impl<T: ToOutput> Generation<'_, T> {
    /// Only these are run: the others can no longer change anything, and bad solutions are the slow
    /// ones, often running into the time limit.
    fn still_owed(&self) -> Vec<IndexedProgram> {
        self.bad.iter().copied().filter(|&(i, _)| self.owes(i)).collect()
    }

    fn owes(&self, solution: usize) -> bool {
        self.failures[solution] < self.target_failures
    }

    fn nothing_is_owed(&self) -> bool {
        !self.bad.iter().any(|&(i, _)| self.owes(i))
    }

    /// Draws `count` candidates from the generators `pick` chooses, and runs the official solution, the
    /// good solutions and `bad` on the new inputs. To stop before a candidate, the caller puts `rng`
    /// back to its `rng_before`, and the candidate is as good as never drawn.
    fn wave(&self, rng: &mut Rng, count: usize, mut pick: impl FnMut(&mut Rng) -> usize, bad: &[IndexedProgram]) -> Result<Vec<Candidate>> {
        let draws: Vec<_> = (0..count)
            .map(|_| {
                let rng_before = rng.clone();
                let generator = pick(rng);
                (generator, rng.next_u64(), rng_before)
            })
            .collect();
        let (subtask, trim_whitespace) = (self.subtask, self.task.trim_whitespace);
        let inputs = parallel_map(&draws, self.runner.workers(), |&(generator, seed, _)| {
            let input = subtask.generate_input(generator, seed, trim_whitespace);
            (stable_hash(&input), input)
        });

        // New to both the earlier tests and the earlier candidates of this wave.
        let mut hashes_in_wave = HashSet::new();
        let mut is_new = |hash: u64| !self.tried_inputs.contains(&hash) && hashes_in_wave.insert(hash);
        let inputs: Vec<_> = inputs.into_iter().map(|input| input.map(|(hash, input)| is_new(hash).then_some((hash, input)))).collect();

        let programs: Vec<_> = std::iter::once(self.official).chain(self.good.iter().chain(bad).map(|&(_, program)| program)).collect();
        let new_inputs = inputs.iter().filter_map(|input| input.as_ref().ok()?.as_ref());
        let jobs: Vec<_> = new_inputs.flat_map(|(_, input)| programs.iter().map(move |&program| Job { program, input })).collect();
        let mut results = self.runner.run_all(&jobs, self.task.limits())?.into_iter();

        let candidates = draws.into_iter().zip(inputs).map(|((generator, seed, rng_before), input)| Candidate {
            generator,
            seed,
            rng_before,
            run: input.map(|input| {
                let (hash, input) = input?;
                let results = results.by_ref().take(programs.len()).collect();
                Some(Run { input, hash, results })
            }),
        });
        Ok(candidates.collect())
    }

    fn write_problem_file(&self, name: &str, contents: &str) -> Result<()> {
        let path = self.task.problem_path.join(name);
        std::fs::write(&path, contents).map_err(Error::io(&path))
    }

    /// The official or a good solution failing is an error. Returns the official output and the
    /// solutions of `bad` that failed.
    fn judge(&self, generator: usize, input: &str, results: Vec<RunResult>, bad: &[IndexedProgram]) -> Result<(String, Vec<usize>)> {
        let mut results = results.into_iter();
        let official_output = match results.next().unwrap_or(RunResult::Crashed).official_output("generation phase", generator + 1) {
            Ok(output) => self.task.normalise_output(output),
            Err(err) => return self.write_problem_file("failing_test.in", input).and(Err(err)),
        };
        if !(self.task.checker)(input, &official_output, &official_output) {
            self.write_problem_file("failing_test.in", input)?;
            return Err(Error::SolutionFailed {
                test_path: "generation phase".to_owned(),
                gen_id: generator + 1,
            });
        }
        let accepted = |result: &RunResult| matches!(result, RunResult::Ok { output, .. } if (self.task.checker)(input, &official_output, output));

        for (&(i, _), result) in self.good.iter().zip(results.by_ref()) {
            if accepted(&result) {
                continue;
            }
            self.write_problem_file("failing_test.in", input)?;
            self.write_problem_file("failing_test_correct_output.out", &official_output)?;
            let wrong_output = self.task.problem_path.join("failing_test_wrong_output.out");
            match &result {
                RunResult::Ok { output, .. } => self.write_problem_file("failing_test_wrong_output.out", output)?,
                _ if wrong_output.is_file() => std::fs::remove_file(&wrong_output).map_err(Error::io(&wrong_output))?,
                _ => {}
            }
            return Err(Error::PartialSolutionFailsSubtask {
                partial_number: i + 1,
                subtask_number: self.subtask_idx + 1,
                subtask_name: self.subtask.name.clone(),
                partial_name: self.task.solutions[i].name.clone(),
                verdict: match result {
                    RunResult::Ok { .. } => "WA",
                    RunResult::TimedOut => "TLE",
                    RunResult::MemoryLimitExceeded => "MLE",
                    RunResult::Crashed => "RTE",
                }
                .to_owned(),
                gen_id: generator + 1,
            });
        }

        let failed = bad.iter().zip(results).filter(|(_, result)| !accepted(result)).map(|(&(i, _), _)| i).collect();
        Ok((official_output, failed))
    }

    /// Returns the test, and whether it counted for a solution that still owed failures.
    fn try_out(&mut self, generator: usize, seed: u64, run: Run, bad: &[IndexedProgram]) -> Result<(GeneratedTest, bool)> {
        self.tried_inputs.insert(run.hash);
        let (output, failed) = self.judge(generator, &run.input, run.results, bad)?;
        let mut counted = false;
        for i in failed {
            if self.owes(i) {
                self.failures[i] += 1;
                counted = true;
            }
        }
        let test = GeneratedTest {
            generator,
            seed,
            input: run.input,
            output,
        };
        Ok((test, counted))
    }

    fn stress_test(&self, rng: &mut Rng) -> Result<()> {
        for generator in (0..self.subtask.num_generators()).filter(|_| self.subtask.stress_tests > 0) {
            info!("Stress testing generator {generator}");
            let progress_bar = ScopedProgressBar::new(&self.task.logger, self.subtask.stress_tests as u64);
            let mut left = self.subtask.stress_tests;
            while left > 0 {
                let wave = self.wave(rng, left.min(self.runner.workers()), |_rng| generator, &[])?;
                left -= wave.len();
                for candidate in wave {
                    if let Some(run) = reached(candidate.run) {
                        self.judge(generator, &run.input, run.results, &[])?;
                    }
                    progress_bar.inc(1);
                }
            }
        }
        Ok(())
    }

    /// Kept whatever the bad solutions do, though failures on them count.
    fn generate_initial_tests(&mut self, rng: &mut Rng, found_progress_bar: &ScopedProgressBar) -> Result<()> {
        for (generator, &needed) in self.subtask.initial_counts.iter().enumerate() {
            let (mut got, mut repeats) = (0, 0);
            while got < needed && repeats < MAX_REPEATED_TESTS {
                let owed = self.still_owed();
                for candidate in self.wave(rng, (needed - got).min(self.runner.workers()), |_rng| generator, &owed)? {
                    if repeats == MAX_REPEATED_TESTS {
                        *rng = candidate.rng_before;
                        break;
                    }
                    let Some(run) = reached(candidate.run) else {
                        repeats += 1;
                        continue;
                    };
                    let (test, _) = self.try_out(generator, candidate.seed, run, &owed)?;
                    self.tests.push(test);
                    found_progress_bar.inc(1);
                    got += 1;
                }
            }
            if got < needed {
                warn!(
                    "Generator {generator} of subtask {} produced only {got} of {needed} tests, because it kept repeating tests it had already generated.",
                    self.subtask_idx + 1
                );
            }
        }
        Ok(())
    }

    /// Kept only if they break a solution that still owes failures. The target is per solution, as one
    /// test rarely breaks all of them.
    fn hunt_for_failures(&mut self, rng: &mut Rng, found_progress_bar: &ScopedProgressBar, tries_progress_bar: &ScopedProgressBar) -> Result<()> {
        let subtask = self.subtask;
        let mut tries = 0;
        loop {
            let owed = self.still_owed();
            if owed.is_empty() || tries >= self.task.max_tries || subtask.num_generators() == 0 {
                return Ok(());
            }

            // No more than the tries left, so only finding every failure can end the hunt mid-wave.
            let wave_size = self.runner.workers().min(self.task.max_tries - tries);
            for candidate in self.wave(rng, wave_size, |rng| subtask.pick_generator(rng).unwrap_or_default(), &owed)? {
                if self.nothing_is_owed() {
                    *rng = candidate.rng_before;
                    return Ok(());
                }
                tries += 1;
                tries_progress_bar.inc(1);
                let Some(run) = reached(candidate.run) else { continue };

                let (test, broke_a_solution) = self.try_out(candidate.generator, candidate.seed, run, &owed)?;
                if broke_a_solution {
                    self.tests.push(test);
                    tries = 0;
                    tries_progress_bar.reset();
                    found_progress_bar.inc(1);
                    found_progress_bar.set_length(found_progress_bar.length().unwrap_or(0).max(found_progress_bar.position()));
                }
            }
        }
    }

    /// A bad solution that failed nothing passes the subtask, which is an error now rather than after
    /// the remaining subtasks. Falling short of the target only weakens the tests, so that is logged.
    fn report_failures(&self) -> Result<()> {
        if let Some(&(i, _)) = self.bad.iter().find(|&&(i, _)| self.failures[i] == 0) {
            return Err(self.task.passes_extra_subtask(i, self.subtask_idx));
        }

        for &(i, _) in self.bad.iter().filter(|&&(i, _)| self.owes(i)) {
            error!(
                "Subtask {} ({}) only has {} of the {} tests that partial solution {} ({}) is supposed to fail.",
                self.subtask_idx + 1,
                self.subtask.name,
                self.failures[i],
                self.target_failures,
                i + 1,
                self.task.solutions[i].name
            );
        }
        Ok(())
    }
}
