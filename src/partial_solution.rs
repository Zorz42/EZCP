use crate::create_tests::GeneratedTest;
use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use crate::{Result, Task, ToOutput};
use console::style;
use std::collections::{BTreeMap, HashSet};
use std::fmt::{Display, Write as _};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    Ok,
    TimedOut,
    Crashed,
    WrongAnswer,
}

impl Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Ok => return write!(f, "{}", style("OK").green().bright().bold()),
            Self::TimedOut => "TLE",
            Self::Crashed => "RTE",
            Self::WrongAnswer => "WA",
        };
        write!(f, "{}", style(text).red().bright().bold())
    }
}

impl<T: ToOutput> Task<T> {
    /// Runs a solution on every test and returns the subtasks it passed.
    pub(crate) fn run_partial_solution(&self, tests: &[Vec<GeneratedTest>], cpp_runner: &mut CppRunner, program: ProgramHandle, lines_of_code: usize) -> Result<HashSet<usize>> {
        cpp_runner.clear_tasks();
        let handles: Vec<Vec<_>> = tests
            .iter()
            .map(|subtask_tests| {
                subtask_tests
                    .iter()
                    .map(|test| (cpp_runner.add_task(program, Arc::clone(&test.input), self.time_limit), &test.output))
                    .collect()
            })
            .collect();
        cpp_runner.run_tasks(Some(&self.logger))?;

        let mut passed_subtasks = HashSet::new();
        let mut results_text = String::new();
        for (subtask_idx, subtask_handles) in handles.into_iter().enumerate() {
            // `None` once a test has failed: no time is reported then.
            let mut max_time = Some(0);
            let mut verdicts = BTreeMap::new();
            for (handle, official_output) in subtask_handles {
                let input = cpp_runner.take_input(handle);
                let verdict = match cpp_runner.get_result(handle) {
                    RunResult::Ok(time, output) => {
                        max_time = max_time.map(|slowest| slowest.max(time));
                        if (self.checker)(&input, official_output, &output) { Verdict::Ok } else { Verdict::WrongAnswer }
                    }
                    RunResult::TimedOut => {
                        max_time = None;
                        Verdict::TimedOut
                    }
                    RunResult::Crashed => {
                        max_time = None;
                        Verdict::Crashed
                    }
                };
                *verdicts.entry(verdict).or_insert(0) += 1;
            }

            write!(results_text, "\n- Subtask {}: ", subtask_idx + 1).ok();
            for (verdict, count) in &verdicts {
                write!(results_text, "{verdict} ({count}) ").ok();
            }
            if let Some(max_time) = max_time {
                write!(results_text, "{max_time}ms").ok();
            }
            if verdicts.keys().eq([&Verdict::Ok]) {
                passed_subtasks.insert(subtask_idx);
            }
        }

        let got_points: i32 = passed_subtasks.iter().map(|&idx| self.subtasks[idx].points).sum();
        let total_points: i32 = self.subtasks.iter().map(|subtask| subtask.points).sum();
        self.log_result(&format!("Points {got_points}/{total_points}"))?;
        self.log_result(&format!("Lines of code: {lines_of_code}"))?;
        self.log_result(&format!("Results: {results_text}"))?;
        Ok(passed_subtasks)
    }
}
