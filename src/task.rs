use crate::subtask::Subtask;
use crate::{Error, Input, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;
use console::style;
use indicatif::{MultiProgress, ProgressBar};
use indicatif_log_bridge::LogWrapper;
use log::{debug, error, info, warn, LevelFilter};
use crate::archiver::archive_files;
use crate::logger_format::logger_format;
use crate::partial_solution::run_partial_solution;
use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;

pub static LOGGER_INIT: Once = Once::new();

// Convert a Path to an owned String for error contexts and logs
fn path_str(p: &Path) -> String { p.to_string_lossy().into_owned() }

/// This struct represents an entire task.
/// You can add subtasks, partial solutions and set the time limit.
/// Once you are done, you can create tests for the task.
pub struct Task {
    name: String,
    // path to the folder with tests
    pub tests_path: PathBuf,
    // time limit in seconds
    pub time_limit: f32,
    // path to the zip file with tests
    pub tests_archive_path: PathBuf,
    // two closures that tells what should the input/output file be named for a given test
    // input to the closure is (test_id, subtask_id, test_id_in_subtask)
    pub get_input_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    pub get_output_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    build_folder_path: PathBuf,
    subtasks: Vec<Subtask>,
    // source code of the solution
    pub solution_source: String,
    // Source of the solution and set of subtasks that the solution passes.
    partial_solutions: Vec<(String, HashSet<usize>)>,

    pub debug_level: LevelFilter,
    logger: MultiProgress,
}

impl Task {
    /// This function creates a new task with the given name and path.
    /// The path should be a relative path to the task folder in which the tests will be generated.
    /// The solution should be at `solution_path` which is `path`/solution.cpp by default but can be changed.
    #[must_use]
    pub fn new(name: &str, path: &Path) -> Self {
        let build_folder_path = path.join("build");
        Self {
            name: name.to_owned(),
            tests_path: path.join("tests"),
            tests_archive_path: path.join("tests.zip"),
            get_input_file_name: Box::new(|test_id, subtask_id, _test_id_in_subtask| format!("test.{:02}.{:03}.in", subtask_id+1, test_id+1)),
            get_output_file_name: Box::new(|test_id, subtask_id, _test_id_in_subtask| format!("test.{:02}.{:03}.out", subtask_id+1, test_id+1)),
            build_folder_path,
            time_limit: 5.0,
            subtasks: Vec::new(),
            partial_solutions: Vec::new(),
            debug_level: LevelFilter::Info,
            logger: MultiProgress::new(),
            solution_source: String::new(),
        }
    }

    pub(crate) fn get_input_file_path(&self, test_id: i32, subtask_id: i32, test_id_in_subtask: i32) -> PathBuf {
        self.tests_path.join((self.get_input_file_name)(test_id, subtask_id, test_id_in_subtask))
    }

    pub(crate) fn get_output_file_path(&self, test_id: i32, subtask_id: i32, test_id_in_subtask: i32) -> PathBuf {
        self.tests_path.join((self.get_output_file_name)(test_id, subtask_id, test_id_in_subtask))
    }

    /// This function adds a subtask to the task.
    /// The subtask must be ready as it cannot be modified after it is added to the task.
    /// The function returns the index of the subtask.
    #[must_use]
    pub fn add_subtask(&mut self, mut subtask: Subtask) -> usize {
        subtask.number = self.subtasks.len();
        self.subtasks.push(subtask);
        self.subtasks.len() - 1
    }

    /// This function adds a dependency between two subtasks.
    /// A dependency means, that the first subtask must be solved before the second subtask.
    /// In practice that means that all the tests from the dependency subtask will be added before the tests from the subtask.
    /// Dependencies apply recursively but do not duplicate tests.
    /// The subtask must be added to the task before this function is called.
    pub fn add_subtask_dependency(&mut self, subtask: usize, dependency: usize) {
        assert!(subtask < self.subtasks.len(), "subtask index out of bounds");
        assert!(dependency < subtask, "dependency must be less than subtask to avoid cycles");
        self.subtasks[subtask].dependencies.push(dependency);
    }

    /// This function adds a partial solution to the task.
    /// A partial solution is a solution that only solves a subset of subtasks.
    /// When the task is generated, the partial solution will be run on all tests of the subtasks it solves.
    /// If the partial solution does not solve the exact subtasks it should, an error will be thrown.
    pub fn add_partial_solution(&mut self, solution_source: String, passes_subtasks: &[usize]) {
        let set = passes_subtasks.iter().copied().collect::<HashSet<_>>();
        self.partial_solutions.push((solution_source, set));
    }

    /// This creates tests and prints the error message if there is an error.
    pub fn create_tests(&mut self) -> Result<()> {
        LOGGER_INIT.call_once(|| {
            let mut builder = env_logger::builder();
            builder.filter(None, self.debug_level);
            builder.format(logger_format);
            let env_logger_instance = builder.build();

            LogWrapper::new(self.logger.clone(), env_logger_instance).try_init().ok();
            log::set_max_level(self.debug_level);
            debug!("Logger initialized with level: {}", self.debug_level);
        });

        let start_time = std::time::Instant::now();
        let res = self.create_tests_inner();
        if let Err(err) = res {
            error!("{}", style(&err).red().bright());
            Err(err)
        } else {
            info!("Elapsed time: {}", style(format!("{:.2}s", start_time.elapsed().as_secs_f32())).bold());
            self.logger.println(format!("{}", style("Success!").green().bright().bold())).ok();
            Ok(())
        }
    }

    /// count how many tests there are in total (if one subtask is a dependency of another, its tests are counted multiple times)
    fn get_num_all_tests(&self) -> i32 {
        let mut num_tests = 0;
        for subtask in &self.subtasks {
            num_tests += self.get_total_tests(subtask);
        }
        num_tests
    }

    fn print_progress(&self, curr: i32, total: i32, text: &str) {
        self.logger.println(format!("[{}/{}] {}", style(curr).bold(), style(total).bold(), style(text).cyan().bold())).ok();
    }

    fn print_title(&self, text: &str) {
        // print title with ===== before and after text
        let mut border_text = String::from(" ");
        for _ in 0..text.len() + 6 { border_text.push('='); }
        self.logger.println(&border_text).ok();
        self.logger.println(format!(" || {} ||", style(text).bold())).ok();
        self.logger.println(&border_text).ok();
    }

    /// This function builds solution and then calls `generate_tests`.
    fn create_tests_inner(&mut self) -> Result<()> {
        self.logger.println("").ok();
        let text = format!("Creating tests for task \"{}\"", self.name);
        self.print_title(&text);

        if self.subtasks.is_empty() {
            warn!("No subtasks defined.");
        }

        // create build directory if it doesn't exist
        if !self.build_folder_path.exists() {
            fs::create_dir_all(&self.build_folder_path)
                .map_err(|err| Error::IOError { err, file: path_str(&self.build_folder_path) })?;
        }

        // check if solution source exists
        if self.solution_source.is_empty() {
            return Err(Error::MissingSolution { });
        }

        // reset subtask input files
        for subtask in &mut self.subtasks {
            for test in &mut subtask.tests {
                test.reset_input_file();
            }
        }

        let mut cpp_runner = CppRunner::new(&self.build_folder_path)?;
        let solution_handle = cpp_runner.add_program(&self.solution_source)?;

        let mut partial_solution_handles = Vec::new();
        for (source, _subtasks) in &mut self.partial_solutions {
            partial_solution_handles.push(cpp_runner.add_program(source)?);
        }

        // create tests directory if it doesn't exist and clear it
        if self.tests_path.exists() {
            fs::remove_dir_all(&self.tests_path)
                .map_err(|err| Error::IOError { err, file: path_str(&self.tests_path) })?;
        }
        fs::create_dir_all(&self.tests_path)
            .map_err(|err| Error::IOError { err, file: path_str(&self.tests_path) })?;

        const TOTAL_STEPS: i32 = 5;
        self.print_progress(1, TOTAL_STEPS, "Generating tests");
        let test_files = self.generate_tests()?;
        self.print_progress(2, TOTAL_STEPS, "Checking generated tests");
        self.check_tests()?;
        self.print_progress(3, TOTAL_STEPS, "Generating test solutions");
        self.generate_test_solutions(&test_files, &mut cpp_runner, solution_handle)?;
        self.print_progress(4, TOTAL_STEPS, "Checking partial solutions");
        self.check_partial_solutions(&test_files, &mut cpp_runner, &partial_solution_handles)?;

        self.print_progress(5, TOTAL_STEPS, "Archiving tests");
        self.archive_tests(&test_files)?;

        let tests_size = fs_extra::dir::get_size(&self.tests_path).unwrap_or(0) as f32 / 1_000_000.0;
        info!("Tests size: {}", style(format!("{tests_size:.2}MB")).bold());

        Ok(())
    }
    
    fn generate_tests(&mut self) -> Result<Vec<Vec<(PathBuf, PathBuf)>>> {
        let num_tests = self.get_num_all_tests();

        // Generate and write tests for each subtask
        let mut curr_test_id = 0;
        let progress_bar = self.logger.add(ProgressBar::new(num_tests as u64));
        let mut test_files = Vec::new();
        for master_subtask in 0..self.subtasks.len() {
            let mut curr_local_test_id = 0;
            let mut tests_written = Vec::new();

            let dependencies = self.get_all_dependencies(&self.subtasks[master_subtask]);
            for subtask_number in dependencies {
                // generate input files paths for all tests because of rust borrow checker
                let mut tests_input_files = Vec::new();
                let num_tests = self.subtasks[subtask_number].tests.len();
                for _ in 0..num_tests {
                    let input_path = self.get_input_file_path(curr_test_id, master_subtask as i32, curr_local_test_id);
                    let output_path = self.get_output_file_path(curr_test_id, master_subtask as i32, curr_local_test_id);
                    tests_input_files.push(input_path.clone());
                    tests_written.push((input_path, output_path));
                    curr_test_id += 1;
                    curr_local_test_id += 1;
                }

                // generate input files for all tests
                for (test, input_file) in &mut self.subtasks[subtask_number].tests.iter_mut().zip(tests_input_files) {
                    test.generate_input(input_file)?;
                    progress_bar.inc(1);
                }
            }

            test_files.push(tests_written);
        }

        self.logger.remove(&progress_bar);

        Ok(test_files)
    }
    
    fn check_tests(&self) -> Result<()> {
        // calculate how many steps there are in total for the progress bar.
        let mut loading_progress_max = 0;
        for subtask in &self.subtasks {
            if subtask.checker.is_some() {
                // and for each check
                loading_progress_max += self.get_total_tests(subtask);
            }
        }
        let progress_bar = self.logger.add(ProgressBar::new(loading_progress_max as u64));

        let mut curr_test_id = 0;
        for (subtask_id, subtask) in self.subtasks.iter().enumerate() {
            let checker = &subtask.checker;
            if let Some(checker) = checker {
                for test_id_in_subtask in 0..self.get_total_tests(subtask) {
                    let input_test_path = self.get_input_file_path(curr_test_id, subtask_id as i32, test_id_in_subtask);
                    let input_str = fs::read_to_string(input_test_path).map_err(|err| Error::IOError { err, file: String::new() })?;
                    checker(Input::new(&input_str))?;
                    curr_test_id += 1;
                    progress_bar.inc(1);
                }
            } else {
                warn!("No checker defined for subtask {}.", subtask.number + 1);
                curr_test_id += self.get_total_tests(subtask);
            }
        }

        self.logger.remove(&progress_bar);
        
        Ok(())
    }
    
    fn generate_test_solutions(&self, test_files: &[Vec<(PathBuf, PathBuf)>], cpp_runner: &mut CppRunner, solution_handle: ProgramHandle) -> Result<()> {
        let mut test_tasks = Vec::new();
        
        // invoke solution on each test
        for subtask in test_files {
            let mut subtask_tasks = Vec::new();
            for (input_file, _output_file) in subtask {
                let test_contents = fs::read_to_string(input_file).map_err(|err| Error::IOError { err, file: input_file.to_str().unwrap_or("?").to_owned() })?;
                let task_handle = cpp_runner.add_task(solution_handle, test_contents, self.time_limit);
                subtask_tasks.push(task_handle);
            }
            test_tasks.push(subtask_tasks);
        }

        cpp_runner.run_tasks(Some(&self.logger))?;

        let mut max_elapsed_time = 0;
        for (subtask, tasks) in test_files.iter().zip(test_tasks) {
            for ((input_file, output_file), task_handle) in subtask.iter().zip(tasks) {
                let test_result = cpp_runner.get_result(task_handle);
                let (elapsed_time, output) = match test_result {
                    RunResult::Ok(elapsed_time, output) => {
                        (elapsed_time, output)
                    }
                    RunResult::TimedOut => {
                        return Err(Error::SolutionTimedOut {
                            test_path: input_file.to_str().unwrap_or("?").to_owned(),
                        });
                    }
                    RunResult::Crashed => {
                        return Err(Error::SolutionFailed {
                            test_path: input_file.to_str().unwrap_or("?").to_owned(),
                        });
                    }
                };
                max_elapsed_time = max_elapsed_time.max(elapsed_time);
                // write output to the output file
                fs::write(output_file, output).map_err(|err| Error::IOError { err, file: output_file.to_str().unwrap_or("?").to_owned() })?;
            }
        }
        info!("Solution time: {max_elapsed_time}ms");
        cpp_runner.clear_tasks();
        
        Ok(())
    }
    
    fn check_partial_solutions(&self, test_files: &[Vec<(PathBuf, PathBuf)>], cpp_runner: &mut CppRunner, partial_solution_handles: &[ProgramHandle]) -> Result<()> {
        for ((partial_id, (_source, passing_subtasks)), program_handle) in self.partial_solutions.iter().enumerate().zip(partial_solution_handles.iter()) {
            info!("Running partial solution {}:", partial_id + 1);
            
            let subtasks_passed = run_partial_solution(test_files, cpp_runner, *program_handle, &self.logger, self.time_limit)?;

            for subtask_id in 0..self.subtasks.len() {
                let subtask_failed = !subtasks_passed.contains(&subtask_id);
                
                if subtask_failed && passing_subtasks.contains(&subtask_id) {
                    return Err(Error::PartialSolutionFailsSubtask {
                        partial_number: partial_id + 1,
                        subtask_number: subtask_id + 1,
                    });
                }

                if !subtask_failed && !passing_subtasks.contains(&subtask_id) {
                    return Err(Error::PartialSolutionPassesExtraSubtask {
                        partial_number: partial_id + 1,
                        subtask_number: subtask_id + 1,
                    });
                }
            }
        }
        
        Ok(())
    }
    
    /// Archive all tests into a zip file
    fn archive_tests(&self, test_files: &[Vec<(PathBuf, PathBuf)>]) -> Result<()> {
        let mut test_files_vec = Vec::new();
        for subtask in test_files {
            for (input_file, output_file) in subtask {
                test_files_vec.push(input_file.clone());
                test_files_vec.push(output_file.clone());
            }
        }
        
        archive_files(&test_files_vec, &self.tests_archive_path, &self.logger)?;

        Ok(())
    }

    /// Get number of tests for a subtask (including dependencies)
    fn get_total_tests(&self, subtask: &Subtask) -> i32 {
        let dependencies = self.get_all_dependencies(subtask);
        let mut result = 0;
        for dependency in dependencies {
            result += self.subtasks[dependency].tests.len() as i32;
        }
        result
    }

    /// Get all dependencies, even dependencies of dependencies and so on
    fn get_all_dependencies(&self, subtask: &Subtask) -> Vec<usize> {
        let mut subtask_visited = vec![false; self.subtasks.len()];
        self.get_all_dependencies_inner(subtask.number, &mut subtask_visited)
    }

    /// A simple dfs to get all dependencies
    fn get_all_dependencies_inner(&self, subtask_number: usize, subtask_visited: &mut Vec<bool>) -> Vec<usize> {
        // check if subtask has already been visited
        if subtask_visited[subtask_number] {
            return Vec::new();
        }
        subtask_visited[subtask_number] = true;

        let mut result = Vec::new();
        for dependency in &self.subtasks[subtask_number].dependencies {
            result.append(&mut self.get_all_dependencies_inner(*dependency, subtask_visited));
        }

        result.push(subtask_number);

        result
    }
}
