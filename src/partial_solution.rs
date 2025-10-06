use std::collections::{BTreeMap, HashSet};
use std::fmt::Display;
use std::fs;
use std::path::PathBuf;
use console::style;
use indicatif::MultiProgress;
use log::info;
use crate::{Error, Result};
use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;

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

impl TestResult {
    pub const fn from(result: &RunResult) -> Self {
        match result {
            RunResult::Ok(_, _) => Self::Ok,
            RunResult::TimedOut => Self::TimedOut,
            RunResult::Crashed => Self::Crashed,
        }
    }
}

/// This function takes an executable file and a list of test files.
/// It runs the executable on each test file and compares the output with the expected output.
/// It returns a set of subtasks that passed.
pub fn run_partial_solution(test_files: &[Vec<(PathBuf, PathBuf)>], cpp_runner: &mut CppRunner, program_handle: ProgramHandle, logger: &MultiProgress, time_limit: f32) -> Result<HashSet<usize>> {
    let mut test_handles = Vec::new();
    let mut passed_subtasks = HashSet::new();
    
    for subtask_tests in test_files {
        let mut test_handles_element = Vec::new();
        for (input_file, output_file) in subtask_tests {
            let input_data = fs::read_to_string(input_file)
                .map_err(|err| Error::IOError { err, file: input_file.to_str().unwrap_or("???").to_owned() })?;
            let handle = cpp_runner.add_task(program_handle, input_data, time_limit);

            test_handles_element.push((handle, output_file.clone()));
        }
        test_handles.push(test_handles_element);
    }

    cpp_runner.run_tasks(Some(logger))?;

    let mut results_text = String::new();
    for (subtask_id, subtask_test_handles) in test_handles.iter().enumerate() {
        let mut max_time = Some(0);
        // count, which result was returned by how many tests
        let mut results = BTreeMap::new();
        for (handle, output_file) in subtask_test_handles {
            let run_result = cpp_runner.get_result(*handle);
            let mut test_result = TestResult::from(&run_result);

            match run_result {
                RunResult::Ok(time, program_output) => {
                    if max_time.is_some() {
                        max_time = Some(i32::max(max_time.unwrap(), time));
                    }

                    let correct_output = fs::read_to_string(output_file)
                        .map_err(|err| Error::IOError { err, file: output_file.to_str().unwrap_or("???").to_owned() })?;
                    if !are_outputs_equal(&correct_output, &program_output) {
                        test_result = TestResult::WrongAnswer;
                    }
                }
                RunResult::TimedOut | RunResult::Crashed => {
                    max_time = None;
                }
            }

            // increment the count for the result
            // keys are strings, because enum has time in the Ok variant
            results
                .entry(test_result)
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }

        results_text += "\n";
        results_text += &format!("- Subtask {}: ", subtask_id + 1);
        for (result, count) in &results {
            results_text += &format!("{result} ({count}) ");
        }

        if let Some(max_time) = max_time {
            results_text += &format!("{max_time}ms");
        }

        if results.len() == 1 && results.contains_key(&TestResult::Ok) {
            passed_subtasks.insert(subtask_id);
        }
    }

    info!("Results: {results_text}");
    
    Ok(passed_subtasks)
}

/// Compares if two outputs have similar contents.
/// It ignores whitespace.
fn are_outputs_equal(output1: &str, output2: &str) -> bool {
    let output1_it = output1.split_whitespace();
    let output2_it = output2.split_whitespace();

    let mut output1 = Vec::new();
    let mut output2 = Vec::new();

    for i in output1_it {
        output1.push(i.to_owned());
    }

    for i in output2_it {
        output2.push(i.to_owned());
    }

    output1 == output2
}