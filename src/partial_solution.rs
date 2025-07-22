use crate::progress_bar::{ANSI_RESET, ANSI_GREEN, ANSI_BOLD, ANSI_RED};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use crate::logger::Logger;
use crate::runner::solution_runner::{SolutionRunner, TestResult};
use crate::{Error, Result};

/// This function takes an executable file and a list of test files.
/// It runs the executable on each test file and compares the output with the expected output.
/// It returns a set of subtasks that passed.
pub fn run_partial_solution(test_files: &Vec<Vec<(PathBuf, PathBuf)>>, exe_path: &Path, logger: &Logger, build_folder: &Path, time_limit: f32) -> Result<HashSet<usize>> {
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

    solution_runner.run_tasks(logger, build_folder);

    for (subtask_id, subtask_test_handles) in test_handles.iter().enumerate() {
        let mut subtask_failed = false;
        let mut max_time = Some(0);
        let mut verdict = format!("{ANSI_BOLD}{ANSI_GREEN}OK{ANSI_RESET}");
        for (handle, output_file, program_output_file) in subtask_test_handles {
            let result = solution_runner.get_result(*handle)?;

            match result {
                TestResult::Ok(time) => {
                    if max_time.is_some() {
                        max_time = Some(i32::max(max_time.unwrap(), time));
                    }
                }
                TestResult::TimedOut | TestResult::Crashed => {
                    verdict = if result == TestResult::TimedOut { format!("{ANSI_BOLD}{ANSI_RED}TLE{ANSI_RESET}") } else { format!("{ANSI_BOLD}{ANSI_RED}RTE{ANSI_RESET}") };
                    max_time = None;
                    subtask_failed = true;
                }
            }

            if !subtask_failed && !are_files_equal(program_output_file, output_file)? {
                verdict = format!("{ANSI_BOLD}{ANSI_RED}WA{ANSI_RESET}");
                subtask_failed = true;
            }
        }

        logger.log(format!("- Subtask {}: {verdict} ", subtask_id + 1));
        if let Some(max_time) = max_time {
            logger.log(format!("{max_time}ms"));
        }
        logger.log("\n");
        
        if !subtask_failed {
            passed_subtasks.insert(subtask_id);
        }
    }
    
    Ok(passed_subtasks)
}

/// Compares if two file have equal contents.
/// It ignores whitespace.
pub fn are_files_equal(file1: &PathBuf, file2: &PathBuf) -> Result<bool> {
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