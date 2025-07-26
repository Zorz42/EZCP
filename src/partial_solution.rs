use std::collections::{BTreeMap, HashSet};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use console::style;
use indicatif::MultiProgress;
use log::info;
use crate::runner::solution_runner::{build_timer, SolutionRunner, RunResult};
use crate::{Error, Result};

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
            RunResult::Ok(_) => Self::Ok,
            RunResult::TimedOut => Self::TimedOut,
            RunResult::Crashed => Self::Crashed,
        }
    }
}

/// This function takes an executable file and a list of test files.
/// It runs the executable on each test file and compares the output with the expected output.
/// It returns a set of subtasks that passed.
pub fn run_partial_solution(test_files: &Vec<Vec<(PathBuf, PathBuf)>>, exe_path: &Path, logger: &MultiProgress, build_folder: &Path, time_limit: f32) -> Result<HashSet<usize>> {
    let mut test_handles = Vec::new();
    let mut solution_runner = SolutionRunner::new();
    let mut passed_subtasks = HashSet::new();
    
    for subtask_tests in test_files {
        let mut test_handles_element = Vec::new();
        for (input_file, output_file) in subtask_tests {
            let temp_output_file = build_folder.join(output_file.file_name().unwrap()).with_extension("out");

            let handle = solution_runner.add_task(exe_path.to_path_buf(), input_file.clone(), temp_output_file.clone(), time_limit);

            test_handles_element.push((handle, output_file.clone(), temp_output_file));
        }
        test_handles.push(test_handles_element);
    }

    let timer_path = build_timer(build_folder)?;
    solution_runner.run_tasks(logger, &timer_path);

    let mut results_text = String::new();
    for (subtask_id, subtask_test_handles) in test_handles.iter().enumerate() {
        let mut max_time = Some(0);
        // count, which result was returned by how many tests
        let mut results = BTreeMap::new();
        for (handle, output_file, program_output_file) in subtask_test_handles {
            let run_result = solution_runner.get_result(*handle)?;
            let mut test_result = TestResult::from(&run_result);

            match run_result {
                RunResult::Ok(time) => {
                    if max_time.is_some() {
                        max_time = Some(i32::max(max_time.unwrap(), time));
                    }
                }
                RunResult::TimedOut | RunResult::Crashed => {
                    max_time = None;
                }
            }

            if !are_files_equal(program_output_file, output_file)? {
                test_result = TestResult::WrongAnswer;
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

/// Compares if two file have equal contents.
/// It ignores whitespace.
fn are_files_equal(file1: &PathBuf, file2: &PathBuf) -> Result<bool> {
    let file1_str = file1.to_str().unwrap_or("???").to_owned();
    let file2_str = file2.to_str().unwrap_or("???").to_owned();
    let file1 = std::fs::read_to_string(file1).map_err(|err| Error::IOError { err, file: file1_str })?;
    let file2 = std::fs::read_to_string(file2).map_err(|err| Error::IOError { err, file: file2_str })?;

    let file1_it = file1.split_whitespace();
    let file2_it = file2.split_whitespace();

    let mut file1 = Vec::new();
    let mut file2 = Vec::new();

    for i in file1_it {
        file1.push(i.to_owned());
    }

    for i in file2_it {
        file2.push(i.to_owned());
    }

    Ok(file1 == file2)
}