use crate::generate::GeneratedTest;
use crate::progress::ScopedProgressBar;
use crate::report::SubtaskResult;
use crate::verdict::Verdict;
use crate::{Result, Task, ToOutput};
use cpprunner::{RunResult, Runner};
use std::fmt::{Display, Write as _};

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
    pub(crate) fn run_solution_on_tests(&self, tests: &[Vec<GeneratedTest>], runner: &Runner, program: &str, expected_to_fail: &dyn Fn(usize) -> bool) -> Result<Vec<SubtaskResult>> {
        let runs = tests
            .iter()
            .map(|subtask_tests| {
                subtask_tests
                    .iter()
                    .map(|test| runner.spawn_checked(program, test.input.clone(), test.output.clone(), &self.checker, self.limits))
                    .collect::<cpprunner::Result<Vec<_>>>()
            })
            .collect::<cpprunner::Result<Vec<_>>>()?;

        let progress_bar = ScopedProgressBar::new(&self.logger, tests.iter().map(Vec::len).sum::<usize>() as u64);
        let mut results = Vec::new();
        for (subtask_idx, runs) in runs.into_iter().enumerate() {
            let stops_early = self.fail_fast && expected_to_fail(subtask_idx);
            let mut result = SubtaskResult::default();
            let count = runs.len();
            // Waited for in order, so the report does not depend on the order the runs finish in.
            // Breaking out drops, and so cancels, the subtask's remaining runs.
            for (test_idx, run) in runs.into_iter().enumerate() {
                let run = run.wait()?;
                progress_bar.inc(1);
                let verdict = Verdict::from(&run);
                *result.verdicts.entry(verdict).or_insert(0) += 1;
                if let RunResult::Ok { time_ms, memory_kb, output: true } = run {
                    result.time_ms = result.time_ms.max(time_ms);
                    result.memory_kb = result.memory_kb.max(memory_kb);
                }
                if verdict != Verdict::Ok && result.first_failure.is_none() {
                    result.first_failure = Some(test_idx);
                    if stops_early {
                        result.skipped = count - test_idx - 1;
                        progress_bar.inc(result.skipped as u64);
                        break;
                    }
                }
            }
            results.push(result);
        }
        drop(progress_bar);

        let got_points: i32 = results.iter().zip(&self.subtasks).filter(|(result, _)| result.passed()).map(|(_, subtask)| subtask.points).sum();
        let total_points: i32 = self.subtasks.iter().map(|subtask| subtask.points).sum();
        let mut results_text = String::new();
        for (subtask_idx, result) in results.iter().enumerate() {
            write!(results_text, "\n- Subtask {}: {result}", subtask_idx + 1).ok();
        }
        self.log_result(&format!("Points {got_points}/{total_points}"))?;
        self.log_result(&format!("Lines of code: {}", program.split('\n').count()))?;
        self.log_result(&format!("Results: {results_text}"))?;
        Ok(results)
    }
}
