use crate::create_tests::GeneratedTest;
use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use crate::{Result, Task, ToOutput};
use console::style;
use std::collections::{BTreeMap, HashSet};
use std::fmt::{Display, Write as _};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum TestResult {
    Ok = 0,
    TimedOut = 1,
    Crashed = 2,
    WrongAnswer = 3,
}

impl Display for TestResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = match self {
            Self::Ok => style("OK").green().bright().bold(),
            Self::TimedOut => style("TLE").red().bright().bold(),
            Self::Crashed => style("RTE").red().bright().bold(),
            Self::WrongAnswer => style("WA").red().bright().bold(),
        };
        write!(f, "{val}")
    }
}

impl From<&RunResult> for TestResult {
    fn from(result: &RunResult) -> Self {
        match result {
            RunResult::Ok(_, _) => Self::Ok,
            RunResult::TimedOut => Self::TimedOut,
            RunResult::Crashed => Self::Crashed,
        }
    }
}

impl<T: ToOutput> Task<T> {
    /// Runs one solution on every generated test and reports which subtasks it
    /// passed.
    ///
    /// The tests are the ones held in memory rather than files on disk, because
    /// in seed mode there are no files: nothing about judging a solution depends
    /// on the tests having been written out.
    pub(crate) fn run_partial_solution(&self, tests: &[Vec<GeneratedTest>], cpp_runner: &mut CppRunner, program_handle: ProgramHandle, lines_of_code: usize) -> Result<HashSet<usize>> {
        cpp_runner.clear_tasks();
        let mut test_handles = Vec::new();
        let mut passed_subtasks = HashSet::new();

        for subtask_tests in tests {
            let mut test_handles_element = Vec::new();
            for test in subtask_tests {
                let handle = cpp_runner.add_task(program_handle, Arc::clone(&test.input), self.time_limit);

                test_handles_element.push((handle, Arc::clone(&test.output)));
            }
            test_handles.push(test_handles_element);
        }

        cpp_runner.run_tasks(Some(&self.logger))?;

        let mut got_points = 0;
        let mut total_points = 0;

        let mut results_text = String::new();
        for (subtask_id, subtask_test_handles) in test_handles.iter().enumerate() {
            let mut max_time = Some(0);
            // count, which result was returned by how many tests
            let mut results = BTreeMap::new();
            for (handle, correct_output) in subtask_test_handles {
                // The runner still holds the input we fed the solution, so take it
                // back rather than keeping a second reference to it alive.
                let input_data = cpp_runner.take_input(*handle);

                let run_result = cpp_runner.get_result(*handle);
                let mut test_result = TestResult::from(&run_result);

                match run_result {
                    RunResult::Ok(time, program_output) => {
                        // `None` means some earlier test already failed, and then no
                        // running time is worth reporting for the subtask.
                        max_time = max_time.map(|slowest| slowest.max(time));

                        if !(self.checker)(&input_data, correct_output, &program_output) {
                            test_result = TestResult::WrongAnswer;
                        }
                    }
                    RunResult::TimedOut | RunResult::Crashed => {
                        max_time = None;
                    }
                }

                // increment the count for the result
                // keys are strings, because enum has time in the Ok variant
                results.entry(test_result).and_modify(|count| *count += 1).or_insert(1);
            }

            write!(results_text, "\n- Subtask {}: ", subtask_id + 1).ok();
            for (result, count) in &results {
                write!(results_text, "{result} ({count}) ").ok();
            }

            if let Some(max_time) = max_time {
                write!(results_text, "{max_time}ms").ok();
            }

            if results.len() == 1 && results.contains_key(&TestResult::Ok) {
                passed_subtasks.insert(subtask_id);
                got_points += self.subtasks[subtask_id].points;
            }
            total_points += self.subtasks[subtask_id].points;
        }

        self.log_result(&format!("Points {got_points}/{total_points}"))?;
        self.log_result(&format!("Lines of code: {lines_of_code}"))?;
        self.log_result(&format!("Results: {results_text}"))?;

        Ok(passed_subtasks)
    }
}
