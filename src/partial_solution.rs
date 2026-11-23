use crate::generate::GeneratedTest;
use crate::outcome::{Judged, Verdict};
use crate::progress::ScopedProgressBar;
use crate::report::SubtaskResult;
use crate::{Result, Task, ToOutput};
use cpprunner::Runner;
use std::fmt::{Display, Write as _};
use std::sync::atomic::{AtomicUsize, Ordering};

impl Display for SubtaskResult {
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
    /// Logs how the solution does on every test and returns how it did on each subtask. With
    /// fail-fast, the subtasks `expected_to_fail` names skip their tests after the first failure.
    pub(crate) fn run_solution_on_tests(
        &self,
        tests: &[Vec<GeneratedTest>],
        runner: &Runner,
        program: &str,
        lines_of_code: usize,
        expected_to_fail: &dyn Fn(usize) -> bool,
    ) -> Result<Vec<SubtaskResult>> {
        let runs: Vec<_> = tests
            .iter()
            .enumerate()
            .flat_map(|(subtask_idx, subtask_tests)| subtask_tests.iter().enumerate().map(move |(test_idx, test)| (subtask_idx, test_idx, test)))
            .collect();
        let jobs: Vec<_> = runs
            .iter()
            .map(|&(_, _, test)| Judged {
                program,
                input: &test.input,
                expected: &test.output,
            })
            .collect();

        // Everything up to a subtask's first failed test is run, so the report does not depend on the
        // order the runs finish in.
        let first_failure: Vec<_> = tests.iter().map(|_| AtomicUsize::new(usize::MAX)).collect();
        let stops_early: Vec<_> = (0..tests.len()).map(|subtask_idx| self.fail_fast && expected_to_fail(subtask_idx)).collect();
        let is_past_failure = |subtask_idx: usize, test_idx: usize| stops_early[subtask_idx] && test_idx > first_failure[subtask_idx].load(Ordering::Relaxed);

        let mut outcomes = vec![None; runs.len()];
        let progress_bar = ScopedProgressBar::new(&self.logger, runs.len() as u64);
        self.judge_runs(runner, &jobs, &|run| is_past_failure(runs[run].0, runs[run].1), |run, outcome| {
            progress_bar.inc(1);
            let (subtask_idx, test_idx, _) = runs[run];
            if outcome.is_some_and(|outcome| outcome.verdict != Verdict::Ok) {
                first_failure[subtask_idx].fetch_min(test_idx, Ordering::Relaxed);
            }
            outcomes[run] = outcome;
        })?;
        drop(progress_bar);

        let mut results: Vec<_> = tests.iter().map(|_| SubtaskResult::default()).collect();
        for (&(subtask_idx, test_idx, _), outcome) in runs.iter().zip(outcomes) {
            let result = &mut results[subtask_idx];
            // It may have run anyway, before the failure was known.
            match outcome.filter(|_| !is_past_failure(subtask_idx, test_idx)) {
                Some(outcome) => {
                    *result.verdicts.entry(outcome.verdict).or_insert(0) += 1;
                    result.time_ms = result.time_ms.max(outcome.time_ms);
                    result.memory_kb = result.memory_kb.max(outcome.memory_kb);
                }
                None => result.skipped += 1,
            }
        }
        for (result, first) in results.iter_mut().zip(&first_failure) {
            result.first_failure = Some(first.load(Ordering::Relaxed)).filter(|&first| first != usize::MAX);
        }

        let got_points: i32 = results.iter().zip(&self.subtasks).filter(|(result, _)| result.passed()).map(|(_, subtask)| subtask.points).sum();
        let total_points: i32 = self.subtasks.iter().map(|subtask| subtask.points).sum();
        let mut results_text = String::new();
        for (subtask_idx, result) in results.iter().enumerate() {
            write!(results_text, "\n- Subtask {}: {result}", subtask_idx + 1).ok();
        }
        self.log_result(&format!("Points {got_points}/{total_points}"))?;
        self.log_result(&format!("Lines of code: {lines_of_code}"))?;
        self.log_result(&format!("Results: {results_text}"))?;
        Ok(results)
    }
}
