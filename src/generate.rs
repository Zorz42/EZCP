//! Generating one subtask's tests.
//!
//! Candidates are drawn, built and run a wave at a time on several threads, yet the outcome is that of
//! taking them one by one: a wave is judged in order, and where that order says to stop, the rest of
//! it is forgotten and the `Rng` put back to before them. So the tests do not depend on the threads.

use crate::error::panic_message;
use crate::outcome::{Judged, Outcome, Verdict, official_output};
use crate::parallel::parallel_map;
use crate::progress::ScopedProgressBar;
use crate::report::{FailedTest, Mismatch, excerpt};
use crate::rng::Rng;
use crate::stub::stable_hash;
use crate::subtask::Fault;
use crate::whitespace::trim_whitespace_owned;
use crate::{Error, Result, Subtask, Task, ToOutput};
use cpprunner::{RunHandle, RunResult, Runner};
use log::{error, info, warn};
use std::collections::HashSet;

/// Repeats a generator may produce in its initial batch before it is given up on; one with a small
/// range may never reach its count.
const MAX_REPEATED_TESTS: usize = 100;

pub struct GeneratedTest {
    pub generator: usize,
    pub seed: u64,
    pub input: String,
    pub output: String,
}

/// An index into `Task::solutions`, and the program's source.
type IndexedProgram<'task> = (usize, &'task str);

struct Candidate {
    generator: usize,
    seed: u64,
    /// Where the `Rng` goes back to if the candidate's turn never comes.
    rng_before: Rng,
    /// `None` for an input tried before, which was not run. An error, such as the generator's panic,
    /// counts only once the candidate's turn comes, since an earlier one may already have ended the run.
    run: Result<Option<Run>>,
}

struct Run {
    input: String,
    hash: u64,
    official: RunResult,
    /// Of the good solutions and the bad ones that were run, by index into `Task::solutions`. None if
    /// the official solution did not finish.
    outcomes: Vec<(usize, Outcome)>,
}

struct Generation<'task, T: ToOutput> {
    task: &'task Task<T>,
    runner: &'task Runner,
    subtask_idx: usize,
    subtask: &'task Subtask<T>,
    official: &'task str,
    /// The partial solutions meant to pass this subtask and not yet seen failing it, and those meant
    /// to fail it.
    good: Vec<IndexedProgram<'task>>,
    bad: Vec<IndexedProgram<'task>>,
    /// How many tests each solution meant to fail has to fail; indexed like `Task::solutions`.
    targets: Vec<usize>,
    failures: Vec<usize>,
    tried_inputs: HashSet<u64>,
    tests: Vec<GeneratedTest>,
    mismatches: &'task mut Vec<Mismatch>,
}

/// What `run` reveals: the official output, the solutions meant to pass that failed, and those of the
/// bad ones run that failed.
type Judgement = (String, Vec<(usize, Verdict)>, Vec<usize>);

impl<T: ToOutput> Task<T> {
    pub(crate) fn normalise_output(&self, raw: String) -> String {
        if self.trim_whitespace { trim_whitespace_owned(raw) } else { raw.trim().to_owned() + "\n" }
    }

    pub(crate) fn generate_input(&self, subtask_idx: usize, generator: usize, seed: u64) -> Result<String> {
        self.subtasks[subtask_idx]
            .generate_input(generator, seed, self.trim_whitespace)
            .map_err(|fault| fault.into_error(subtask_idx, generator, seed))
    }

    pub(crate) fn passes_extra_subtask(&self, partial: usize, subtask: usize) -> Error {
        Error::PartialSolutionPassesExtraSubtask {
            subtask_number: subtask + 1,
            partial_number: partial + 1,
            partial_name: self.solutions[partial].name.clone(),
            subtask_name: self.subtasks[subtask].name.clone(),
        }
    }

    pub(crate) fn create_tests_for_subtask(&self, subtask_idx: usize, rng: &mut Rng, runner: &Runner, official: &str, partials: &[&str], mismatches: &mut Vec<Mismatch>) -> Result<Vec<GeneratedTest>> {
        let subtask = &self.subtasks[subtask_idx];
        let (good, bad): (Vec<IndexedProgram>, Vec<IndexedProgram>) = partials.iter().copied().enumerate().partition(|&(i, _)| !self.solutions[i].should_fail(subtask_idx));
        let default_target = subtask.min_failures_per_solution.unwrap_or(self.min_failures_per_solution);
        let targets = self
            .solutions
            .iter()
            .map(|solution| solution.min_failures.get(&subtask_idx).copied().unwrap_or(default_target))
            .collect();
        let progress_target = if bad.is_empty() { 0 } else { default_target };
        let mut generation = Generation {
            task: self,
            runner,
            subtask_idx,
            subtask,
            official,
            good,
            bad,
            targets,
            failures: vec![0; self.solutions.len()],
            tried_inputs: HashSet::new(),
            tests: Vec::new(),
            mismatches,
        };

        let total_initial: usize = subtask.initial_counts.iter().sum();
        let found_progress_bar = ScopedProgressBar::new(&self.logger, (total_initial + progress_target) as u64);
        let tries_progress_bar = ScopedProgressBar::new(&self.logger, self.max_tries as u64);

        generation.stress_test(rng)?;
        generation.generate_initial_tests(rng, &found_progress_bar)?;
        generation.hunt_for_failures(rng, &found_progress_bar, &tries_progress_bar)?;
        if !generation.tests.is_empty() {
            generation.report_failures()?;
        }

        let mut tests = generation.tests;
        rng.shuffle(&mut tests);
        Ok(tests)
    }
}

impl<'task, T: ToOutput> Generation<'task, T> {
    /// Only these are run: the others can no longer change anything, and bad solutions are the slow
    /// ones, often running into the time limit.
    fn still_owed(&self) -> Vec<IndexedProgram<'task>> {
        self.bad.iter().copied().filter(|&(i, _)| self.owes(i)).collect()
    }

    fn owes(&self, solution: usize) -> bool {
        self.failures[solution] < self.targets[solution]
    }

    fn nothing_is_owed(&self) -> bool {
        !self.bad.iter().any(|&(i, _)| self.owes(i))
    }

    /// Draws `count` candidates from the generators `pick` chooses, and runs the official solution on
    /// the new inputs, then the good solutions and `bad` where it finished. To stop before a
    /// candidate, the caller puts `rng` back to its `rng_before`, and the candidate is as good as never
    /// drawn.
    fn wave(&self, rng: &mut Rng, count: usize, mut pick: impl FnMut(&mut Rng) -> usize, bad: &[IndexedProgram]) -> Result<Vec<Candidate>> {
        let draws: Vec<_> = (0..count)
            .map(|_| {
                let rng_before = rng.clone();
                let generator = pick(rng);
                (generator, rng.next_u64(), rng_before)
            })
            .collect();
        let (subtask_idx, subtask, trim_whitespace) = (self.subtask_idx, self.subtask, self.task.trim_whitespace);
        let inputs = parallel_map(&draws, self.runner.workers(), |&(generator, seed, _)| subtask.generate_input(generator, seed, trim_whitespace));

        // New to both the earlier tests and the earlier candidates of this wave.
        let mut hashes_in_wave = HashSet::new();
        let inputs: Vec<Result<Option<(u64, String)>>> = draws
            .iter()
            .zip(inputs)
            .map(|(&(generator, seed, _), input)| {
                let input = input
                    .unwrap_or_else(|panic| Err(Fault::Generator(panic_message(&*panic))))
                    .map_err(|fault| fault.into_error(subtask_idx, generator, seed))?;
                let hash = stable_hash(&input);
                Ok((!self.tried_inputs.contains(&hash) && hashes_in_wave.insert(hash)).then_some((hash, input)))
            })
            .collect();

        let new_inputs: Vec<&str> = inputs.iter().filter_map(|input| Some(input.as_ref().ok()?.as_ref()?.1.as_str())).collect();
        let official_runs = new_inputs
            .iter()
            .map(|&input| self.runner.spawn(self.official, input, self.task.limits()))
            .collect::<cpprunner::Result<Vec<_>>>()?;
        let official = official_runs.into_iter().map(RunHandle::wait).collect::<cpprunner::Result<Vec<_>>>()?;
        let expected: Vec<_> = official
            .iter()
            .map(|result| match result {
                RunResult::Ok { output, .. } => Some(self.task.normalise_output(output.clone())),
                _ => None,
            })
            .collect();

        let partials: Vec<IndexedProgram> = self.good.iter().chain(bad).copied().collect();
        let judged: Vec<(usize, usize, Judged)> = new_inputs
            .iter()
            .zip(&expected)
            .enumerate()
            .filter_map(|(new, (&input, expected))| Some((new, input, expected.as_deref()?)))
            .flat_map(|(new, input, expected)| partials.iter().map(move |&(i, program)| (new, i, Judged { program, input, expected })))
            .collect();
        let jobs: Vec<_> = judged.iter().map(|&(_, _, job)| job).collect();
        let mut outcomes = vec![Vec::new(); new_inputs.len()];
        self.task.judge_runs(self.runner, &jobs, 0, &|_job| false, |job, outcome| {
            if let Some(outcome) = outcome {
                let (new, i, _) = judged[job];
                outcomes[new].push((i, outcome));
            }
        })?;

        let mut runs = official.into_iter().zip(outcomes);
        let candidates = draws.into_iter().zip(inputs).map(|((generator, seed, rng_before), input)| Candidate {
            generator,
            seed,
            rng_before,
            run: input.map(|input| {
                let (hash, input) = input?;
                let (official, outcomes) = runs.next()?;
                Some(Run { input, hash, official, outcomes })
            }),
        });
        Ok(candidates.collect())
    }

    fn write_problem_file(&self, name: &str, contents: &str) -> Result<()> {
        let path = self.task.problem_path.join(name);
        std::fs::write(&path, contents).map_err(Error::io(&path))
    }

    /// The official solution failing is an error.
    fn judge(&self, generator: usize, run: &Run, bad: &[IndexedProgram]) -> Result<Judgement> {
        let official_output = match official_output(run.official.clone(), "generation phase", generator + 1) {
            Ok(output) => self.task.normalise_output(output),
            Err(err) => return self.write_problem_file("failing_test.in", &run.input).and(Err(err)),
        };
        let accepted = self.runner.spawn_check(&self.task.checker, run.input.as_str(), official_output.as_str(), official_output.as_str())?;
        if !accepted.wait()? {
            self.write_problem_file("failing_test.in", &run.input)?;
            return Err(Error::SolutionFailed {
                test_path: "generation phase".to_owned(),
                gen_id: generator + 1,
            });
        }
        let failed = |i: usize| {
            run.outcomes
                .iter()
                .find(|&&(j, _)| j == i)
                .map(|&(_, outcome)| outcome.verdict)
                .filter(|&verdict| verdict != Verdict::Ok)
        };
        let good_failed = self.good.iter().filter_map(|&(i, _)| Some((i, failed(i)?))).collect();
        let bad_failed = bad.iter().filter(|&&(i, _)| failed(i).is_some()).map(|&(i, _)| i).collect();
        Ok((official_output, good_failed, bad_failed))
    }

    /// Records a solution meant to pass the subtask failing a test of it. That ends the run, unless the
    /// task keeps going, when the solution is no longer run on this subtask.
    fn good_failed(&mut self, solution: usize, verdict: Verdict, generator: usize, seed: u64, input: &str, expected: &str) -> Result<()> {
        let Some(&(_, program)) = self.good.iter().find(|&&(i, _)| i == solution) else { return Ok(()) };
        self.good.retain(|&(i, _)| i != solution);
        // Outputs are not kept, so it is run again for its own.
        let output = match self.runner.spawn(program, input, self.task.limits())?.wait()? {
            RunResult::Ok { output, .. } => Some(output),
            _ => None,
        };
        let test = FailedTest {
            generator,
            seed,
            input: excerpt(input),
            expected: excerpt(expected),
            output: output.as_deref().map(excerpt),
        };
        self.mismatches.push(Mismatch::Fails {
            solution,
            subtask: self.subtask_idx,
            verdict,
            test,
        });
        if self.task.keep_going {
            return Ok(());
        }

        self.write_problem_file("failing_test.in", input)?;
        self.write_problem_file("failing_test_correct_output.out", expected)?;
        let wrong_output = self.task.problem_path.join("failing_test_wrong_output.out");
        match &output {
            Some(output) => self.write_problem_file("failing_test_wrong_output.out", output)?,
            None if wrong_output.is_file() => std::fs::remove_file(&wrong_output).map_err(Error::io(&wrong_output))?,
            None => {}
        }
        Err(Error::PartialSolutionFailsSubtask {
            partial_number: solution + 1,
            subtask_number: self.subtask_idx + 1,
            subtask_name: self.subtask.name.clone(),
            partial_name: self.task.solutions[solution].name.clone(),
            verdict: verdict.code().to_owned(),
            gen_id: generator + 1,
        })
    }

    /// Returns the test, and whether it broke a solution: one meant to pass, or a bad one that still
    /// owed failures.
    fn try_out(&mut self, generator: usize, seed: u64, run: Run, bad: &[IndexedProgram]) -> Result<(GeneratedTest, bool)> {
        self.tried_inputs.insert(run.hash);
        let (output, good_failed, bad_failed) = self.judge(generator, &run, bad)?;
        let mut counted = !good_failed.is_empty();
        for (i, verdict) in good_failed {
            self.good_failed(i, verdict, generator, seed, &run.input, &output)?;
        }
        for i in bad_failed {
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

    fn stress_test(&mut self, rng: &mut Rng) -> Result<()> {
        for generator in (0..self.subtask.num_generators()).filter(|_| self.subtask.stress_tests > 0) {
            info!("Stress testing generator {generator}");
            let progress_bar = ScopedProgressBar::new(&self.task.logger, self.subtask.stress_tests as u64);
            let mut left = self.subtask.stress_tests;
            while left > 0 {
                let wave = self.wave(rng, left.min(self.runner.workers()), |_rng| generator, &[])?;
                left -= wave.len();
                for candidate in wave {
                    if let Some(run) = candidate.run? {
                        let (output, good_failed, _) = self.judge(generator, &run, &[])?;
                        for (i, verdict) in good_failed {
                            self.good_failed(i, verdict, generator, candidate.seed, &run.input, &output)?;
                        }
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
                    let Some(run) = candidate.run? else {
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

    /// Kept only if they break a solution: one that still owes failures, or one meant to pass. The
    /// target is per solution, as one test rarely breaks all of them.
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
                let Some(run) = candidate.run? else { continue };

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

    /// A bad solution that failed nothing passes the subtask, which is found now rather than after the
    /// remaining subtasks. Falling short of the target only weakens the tests, so that is logged.
    fn report_failures(&mut self) -> Result<()> {
        let passing: Vec<_> = self.bad.iter().map(|&(i, _)| i).filter(|&i| self.targets[i] > 0 && self.failures[i] == 0).collect();
        for i in passing {
            self.mismatches.push(Mismatch::Passes {
                solution: i,
                subtask: self.subtask_idx,
            });
            if !self.task.keep_going {
                return Err(self.task.passes_extra_subtask(i, self.subtask_idx));
            }
        }

        for &(i, _) in self.bad.iter().filter(|&&(i, _)| self.failures[i] > 0 && self.owes(i)) {
            error!(
                "Subtask {} ({}) only has {} of the {} tests that partial solution {} ({}) is supposed to fail.",
                self.subtask_idx + 1,
                self.subtask.name,
                self.failures[i],
                self.targets[i],
                i + 1,
                self.task.solutions[i].name
            );
        }
        Ok(())
    }
}
