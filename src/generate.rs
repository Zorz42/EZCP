//! Generating the tests of one subtask.
//!
//! Candidates are drawn and run a wave at a time, so that their inputs can be
//! built and the solutions run on several threads. The outcome is nevertheless
//! the one of drawing, building and running them one after another: a wave is
//! judged in order, and when that order says to stop, the candidates after that
//! point are forgotten and the [`Rng`] is put back to where it was before them.
//! The tests therefore do not depend on the number of threads.

use crate::parallel::parallel_map;
use crate::progress::ScopedProgressBar;
use crate::rng::Rng;
use crate::runner::cpp_runner::{CppRunner, Job, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use crate::stub::stable_hash;
use crate::whitespace::trim_whitespace_owned;
use crate::{Error, Result, Subtask, Task, ToOutput};
use log::{error, info, warn};
use std::any::Any;
use std::collections::HashSet;
use std::panic::resume_unwind;

/// How many duplicate tests a generator may produce in its initial batch before
/// it is given up on, since one with a small range may never reach its count.
const MAX_REPEATED_TESTS: usize = 100;

/// A kept test. `generator` and `seed` rebuild `input`, which is what a stub
/// records.
pub struct GeneratedTest {
    pub generator: usize,
    pub seed: u64,
    pub input: String,
    pub output: String,
}

/// A solution's index into `Task::solutions`, and its program.
type IndexedProgram = (usize, ProgramHandle);

/// What a panicking generator leaves behind.
type Panic = Box<dyn Any + Send>;

/// A candidate test that has been drawn, but perhaps not built yet.
struct Draw {
    generator: usize,
    seed: u64,
    /// Where the `Rng` goes back to if this draw should never have happened.
    rng_before: Rng,
}

/// The state of generating one subtask's tests.
struct Generation<'task, T: ToOutput> {
    task: &'task Task<T>,
    runner: &'task CppRunner,
    subtask_idx: usize,
    subtask: &'task Subtask<T>,
    official: ProgramHandle,
    /// The partial solutions meant to pass this subtask, and those meant to fail it.
    good: Vec<IndexedProgram>,
    bad: Vec<IndexedProgram>,
    /// How many tests each bad solution has to fail.
    target_failures: usize,
    /// How many it has failed so far, indexed like `Task::solutions`.
    failures: Vec<usize>,
    tried_inputs: HashSet<u64>,
    tests: Vec<GeneratedTest>,
}

/// The candidates before the first whose generator panicked, and that panic.
/// It has to wait until those before it are judged, as one of them may already
/// end the run or the search.
fn until_first_panic(wave: Vec<(Draw, std::thread::Result<String>)>) -> (Vec<(Draw, String)>, Option<(Draw, Panic)>) {
    let mut built = Vec::with_capacity(wave.len());
    for (draw, input) in wave {
        match input {
            Ok(input) => built.push((draw, input)),
            Err(panic) => return (built, Some((draw, panic))),
        }
    }
    (built, None)
}

impl<T: ToOutput> Task<T> {
    pub(crate) fn normalise_output(&self, raw: String) -> String {
        if self.trim_whitespace { trim_whitespace_owned(raw) } else { raw.trim().to_owned() + "\n" }
    }

    /// # Panics
    /// Panics if `subtask_idx` or `generator` do not exist.
    pub(crate) fn generate_input(&self, subtask_idx: usize, generator: usize, seed: u64) -> String {
        self.subtasks[subtask_idx].generate_input(generator, seed, self.trim_whitespace)
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

        // With the run's rng, so the order is reproducible too.
        let mut tests = generation.tests;
        rng.shuffle(&mut tests);
        Ok(tests)
    }
}

impl<T: ToOutput> Generation<'_, T> {
    /// How many candidates are drawn at a time.
    const fn wave_size(&self) -> usize {
        self.runner.workers()
    }

    /// The bad solutions that have not failed enough tests yet. Only these are
    /// run: the others can no longer change anything, and bad solutions are the
    /// slow ones, since they often run into the time limit.
    fn still_owed(&self) -> Vec<IndexedProgram> {
        self.bad.iter().copied().filter(|&(i, _)| self.owes(i)).collect()
    }

    fn owes(&self, solution: usize) -> bool {
        self.failures[solution] < self.target_failures
    }

    /// Whether every bad solution has failed enough tests.
    fn nothing_is_owed(&self) -> bool {
        !self.bad.iter().any(|&(i, _)| self.owes(i))
    }

    /// Draws `count` candidates, from the generators `pick` chooses, and builds
    /// their inputs.
    fn draw_wave(&self, rng: &mut Rng, count: usize, mut pick: impl FnMut(&mut Rng) -> usize) -> Vec<(Draw, std::thread::Result<String>)> {
        let draws: Vec<Draw> = (0..count)
            .map(|_| {
                let rng_before = rng.clone();
                let generator = pick(rng);
                Draw {
                    generator,
                    seed: rng.next_seed(),
                    rng_before,
                }
            })
            .collect();
        let (subtask, trim_whitespace) = (self.subtask, self.task.trim_whitespace);
        let inputs = parallel_map(&draws, self.runner.workers(), |draw| subtask.generate_input(draw.generator, draw.seed, trim_whitespace));
        draws.into_iter().zip(inputs).collect()
    }

    /// Runs the official solution, the good solutions and `bad`, in that order,
    /// on every input.
    fn run_solutions<'input>(&self, inputs: impl Iterator<Item = &'input str>, bad: &[IndexedProgram]) -> Result<std::vec::IntoIter<Vec<RunResult>>> {
        let programs: Vec<_> = std::iter::once(self.official).chain(self.good.iter().chain(bad).map(|&(_, handle)| handle)).collect();
        let jobs: Vec<_> = inputs.flat_map(|input| programs.iter().map(move |&program| Job { program, input })).collect();
        let mut results = self.runner.run_all(&jobs, self.task.limits())?.into_iter();
        let per_input = (0..jobs.len() / programs.len()).map(|_| results.by_ref().take(programs.len()).collect()).collect::<Vec<_>>();
        Ok(per_input.into_iter())
    }

    fn write_problem_file(&self, name: &str, contents: &str) -> Result<()> {
        let path = self.task.problem_path.join(name);
        std::fs::write(&path, contents).map_err(Error::io(&path))
    }

    /// Judges what [`Generation::run_solutions`] made of a candidate. The
    /// official or a good solution failing is an error. Returns the official
    /// output and those of `bad` that failed.
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

    /// Counts a test for those of the solutions it broke that still owe
    /// failures. Returns whether there were any.
    fn count_failures(&mut self, failed: &[usize]) -> bool {
        let counted: Vec<usize> = failed.iter().copied().filter(|&i| self.owes(i)).collect();
        for &i in &counted {
            self.failures[i] += 1;
        }
        !counted.is_empty()
    }

    fn keep(&mut self, draw: &Draw, input: String, output: String) {
        self.tests.push(GeneratedTest {
            generator: draw.generator,
            seed: draw.seed,
            input,
            output,
        });
    }

    /// Runs every generator through the solutions meant to pass, to find the
    /// ones that hang or break such a solution. The tests are discarded.
    fn stress_test(&self, rng: &mut Rng) -> Result<()> {
        for generator in (0..self.subtask.num_generators()).filter(|_| self.subtask.stress_tests > 0) {
            info!("Stress testing generator {generator}");
            let progress_bar = ScopedProgressBar::new(&self.task.logger, self.subtask.stress_tests as u64);
            let mut left = self.subtask.stress_tests;
            while left > 0 {
                let wave = self.draw_wave(rng, left.min(self.wave_size()), |_rng| generator);
                left -= wave.len();

                let (built, panic) = until_first_panic(wave);
                let results = self.run_solutions(built.iter().map(|(_, input)| input.as_str()), &[])?;
                for ((_, input), results) in built.iter().zip(results) {
                    self.judge(generator, input, results, &[])?;
                    progress_bar.inc(1);
                }
                if let Some((_, panic)) = panic {
                    resume_unwind(panic);
                }
            }
        }
        Ok(())
    }

    /// The initial tests are kept whatever the bad solutions do, but failures
    /// on them still count towards the target.
    fn generate_initial_tests(&mut self, rng: &mut Rng, found_progress_bar: &ScopedProgressBar) -> Result<()> {
        for (generator, &needed) in self.subtask.initial_counts.iter().enumerate() {
            let (mut got, mut repeats) = (0, 0);
            while got < needed && repeats < MAX_REPEATED_TESTS {
                let wave = self.draw_wave(rng, (needed - got).min(self.wave_size()), |_rng| generator);

                // The new inputs of the wave, as far as the generator gets.
                let mut fresh = Vec::with_capacity(wave.len());
                let mut pending_panic = None;
                for (draw, input) in wave {
                    if repeats == MAX_REPEATED_TESTS {
                        // Drawn ahead, but the generator is given up on before it.
                        *rng = draw.rng_before;
                        break;
                    }
                    match input {
                        Ok(input) if self.tried_inputs.insert(stable_hash(&input)) => fresh.push((draw, input)),
                        Ok(_repeated) => repeats += 1,
                        Err(panic) => {
                            pending_panic = Some(panic);
                            break;
                        }
                    }
                }

                let owed = self.still_owed();
                let results = self.run_solutions(fresh.iter().map(|(_, input)| input.as_str()), &owed)?;
                for ((draw, input), results) in fresh.into_iter().zip(results) {
                    let (output, failed) = self.judge(generator, &input, results, &owed)?;
                    self.count_failures(&failed);
                    self.keep(&draw, input, output);
                    found_progress_bar.inc(1);
                    got += 1;
                }
                // Only now: a test before it may already have ended the run.
                if let Some(panic) = pending_panic {
                    resume_unwind(panic);
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

    /// Supplemental tests, kept only if they break a solution that still owes
    /// failures. The target is per solution: requiring one test to break all
    /// of them at once rarely succeeds.
    fn hunt_for_failures(&mut self, rng: &mut Rng, found_progress_bar: &ScopedProgressBar, tries_progress_bar: &ScopedProgressBar) -> Result<()> {
        if self.subtask.num_generators() == 0 {
            return Ok(());
        }

        let mut tries = 0;
        loop {
            let owed = self.still_owed();
            if owed.is_empty() || tries >= self.task.max_tries {
                return Ok(());
            }

            // No more than the tries that are left, so that only having found
            // every failure can end the search in the middle of a wave.
            let subtask = self.subtask;
            let wave = self.draw_wave(rng, self.wave_size().min(self.task.max_tries - tries), |rng| subtask.pick_generator(rng).unwrap_or_default());
            let (built, panic) = until_first_panic(wave);

            // `None` for an input that has been tried before, which is not run.
            let candidates: Vec<_> = built
                .into_iter()
                .map(|(draw, input)| {
                    let is_new = self.tried_inputs.insert(stable_hash(&input));
                    (draw, is_new.then_some(input))
                })
                .collect();
            let mut results = self.run_solutions(candidates.iter().filter_map(|(_, input)| input.as_deref()), &owed)?;

            for (draw, input) in candidates {
                if self.nothing_is_owed() {
                    // Drawn ahead, but the search ends before it.
                    *rng = draw.rng_before;
                    return Ok(());
                }
                tries += 1;
                tries_progress_bar.inc(1);
                // There is a result for every input that was run.
                let Some(input) = input else { continue };
                let Some(results) = results.next() else { continue };

                let (output, failed) = self.judge(draw.generator, &input, results, &owed)?;
                if self.count_failures(&failed) {
                    self.keep(&draw, input, output);
                    tries = 0;
                    tries_progress_bar.reset();
                    found_progress_bar.inc(1);
                    // The number of extra tests is not known in advance.
                    found_progress_bar.set_length(found_progress_bar.length().unwrap_or(0).max(found_progress_bar.position()));
                }
            }

            if let Some((draw, panic)) = panic {
                if self.nothing_is_owed() {
                    *rng = draw.rng_before;
                    return Ok(());
                }
                resume_unwind(panic);
            }
        }
    }

    /// A bad solution with no failures at all will pass the subtask, so that is
    /// reported now rather than after all the remaining subtasks. Falling short
    /// of the target only weakens the tests, so that is just logged.
    fn report_failures(&self) -> Result<()> {
        if let Some(&(i, _)) = self.bad.iter().find(|&&(i, _)| self.failures[i] == 0) {
            return Err(Error::PartialSolutionPassesExtraSubtask {
                subtask_number: self.subtask_idx + 1,
                partial_number: i + 1,
                partial_name: self.task.solutions[i].name.clone(),
                subtask_name: self.subtask.name.clone(),
            });
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
