use crate::generate::GeneratedTest;
use crate::progress::ScopedProgressBar;
use crate::runner::cpp_runner::{CppRunner, Job, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use crate::{Result, Task, ToOutput};
use console::style;
use std::collections::{BTreeMap, HashSet};
use std::fmt::{Display, Write as _};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    Ok,
    TimedOut,
    MemoryLimitExceeded,
    Crashed,
    WrongAnswer,
}

impl Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Ok => return write!(f, "{}", style("OK").green().bright().bold()),
            Self::TimedOut => "TLE",
            Self::MemoryLimitExceeded => "MLE",
            Self::Crashed => "RTE",
            Self::WrongAnswer => "WA",
        };
        write!(f, "{}", style(text).red().bright().bold())
    }
}

/// How a solution did on one test.
#[derive(Clone, Copy)]
struct Outcome {
    verdict: Verdict,
    /// In milliseconds and kilobytes; zero unless the verdict is `Ok`.
    time_ms: i32,
    memory_kb: u64,
}

/// How a solution did on the tests of one subtask.
#[derive(Default)]
struct SubtaskReport {
    verdicts: BTreeMap<Verdict, usize>,
    /// Tests that were not run because an earlier one had failed.
    skipped: usize,
    /// The largest of the tests; meaningless once one has failed.
    time_ms: i32,
    memory_kb: u64,
}

impl SubtaskReport {
    fn passed(&self) -> bool {
        self.skipped == 0 && self.verdicts.keys().eq([&Verdict::Ok])
    }
}

impl Display for SubtaskReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (verdict, count) in &self.verdicts {
            write!(f, "{verdict} ({count}) ")?;
        }
        if self.skipped > 0 {
            write!(f, "skipped ({}) ", self.skipped)?;
        }
        if self.passed() {
            write!(f, "{}ms {:.1}MB", self.time_ms, self.memory_kb as f64 / 1024.0)?;
        }
        Ok(())
    }
}

impl<T: ToOutput> Task<T> {
    fn outcome(&self, result: RunResult, test: &GeneratedTest) -> Outcome {
        let failed = |verdict| Outcome { verdict, time_ms: 0, memory_kb: 0 };
        match result {
            RunResult::Ok { time_ms, memory_kb, output } if (self.checker)(&test.input, &test.output, &output) => Outcome {
                verdict: Verdict::Ok,
                time_ms,
                memory_kb,
            },
            RunResult::Ok { .. } => failed(Verdict::WrongAnswer),
            RunResult::TimedOut => failed(Verdict::TimedOut),
            RunResult::MemoryLimitExceeded => failed(Verdict::MemoryLimitExceeded),
            RunResult::Crashed => failed(Verdict::Crashed),
        }
    }

    /// Runs a solution on every test, logs how it did and returns the subtasks
    /// it passed.
    ///
    /// With [`Task::with_fail_fast`], the tests after the first failed one are
    /// skipped in the subtasks `expected_to_fail` names.
    pub(crate) fn run_solution_on_tests(
        &self,
        tests: &[Vec<GeneratedTest>],
        runner: &CppRunner,
        program: ProgramHandle,
        lines_of_code: usize,
        expected_to_fail: &dyn Fn(usize) -> bool,
    ) -> Result<HashSet<usize>> {
        // `(subtask, test in subtask, test)` of every test, in order.
        let runs: Vec<_> = tests
            .iter()
            .enumerate()
            .flat_map(|(subtask_idx, subtask_tests)| subtask_tests.iter().enumerate().map(move |(test_idx, test)| (subtask_idx, test_idx, test)))
            .collect();
        let jobs: Vec<_> = runs.iter().map(|&(_, _, test)| Job { program, input: &test.input }).collect();

        // The first failed test of each subtask, and whether the subtask's
        // later tests are skipped. Everything up to that test is always run, so
        // what is reported does not depend on the order the runs finish in.
        let first_failure: Vec<_> = tests.iter().map(|_| AtomicUsize::new(usize::MAX)).collect();
        let stops_early: Vec<_> = (0..tests.len()).map(|subtask_idx| self.fail_fast && expected_to_fail(subtask_idx)).collect();
        let is_past_failure = |subtask_idx: usize, test_idx: usize| stops_early[subtask_idx] && test_idx > first_failure[subtask_idx].load(Ordering::Relaxed);

        let mut outcomes = vec![None; runs.len()];
        let progress_bar = ScopedProgressBar::new(&self.logger, runs.len() as u64);
        runner.run_jobs(&jobs, self.limits(), &|run| is_past_failure(runs[run].0, runs[run].1), |run, result| {
            progress_bar.inc(1);
            let (subtask_idx, test_idx, test) = runs[run];
            let outcome = result.map(|result| self.outcome(result, test));
            if outcome.is_some_and(|outcome| outcome.verdict != Verdict::Ok) {
                first_failure[subtask_idx].fetch_min(test_idx, Ordering::Relaxed);
            }
            outcomes[run] = outcome;
        })?;
        drop(progress_bar);

        let mut reports: Vec<_> = tests.iter().map(|_| SubtaskReport::default()).collect();
        for (&(subtask_idx, test_idx, _), outcome) in runs.iter().zip(outcomes) {
            let report = &mut reports[subtask_idx];
            // A test past the failure may have run all the same, before the failure was known.
            match outcome.filter(|_| !is_past_failure(subtask_idx, test_idx)) {
                Some(outcome) => {
                    *report.verdicts.entry(outcome.verdict).or_insert(0) += 1;
                    report.time_ms = report.time_ms.max(outcome.time_ms);
                    report.memory_kb = report.memory_kb.max(outcome.memory_kb);
                }
                None => report.skipped += 1,
            }
        }

        let passed_subtasks: HashSet<usize> = reports.iter().enumerate().filter(|(_, report)| report.passed()).map(|(subtask_idx, _)| subtask_idx).collect();
        let mut results_text = String::new();
        for (subtask_idx, report) in reports.iter().enumerate() {
            write!(results_text, "\n- Subtask {}: {report}", subtask_idx + 1).ok();
        }

        let got_points: i32 = passed_subtasks.iter().map(|&idx| self.subtasks[idx].points).sum();
        let total_points: i32 = self.subtasks.iter().map(|subtask| subtask.points).sum();
        self.log_result(&format!("Points {got_points}/{total_points}"))?;
        self.log_result(&format!("Lines of code: {lines_of_code}"))?;
        self.log_result(&format!("Results: {results_text}"))?;
        Ok(passed_subtasks)
    }
}
