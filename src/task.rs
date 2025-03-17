use crate::progress_bar::{ANSI_GREEN, ANSI_RED};
use crate::logger::Logger;
use crate::progress_bar::{clear_progress_bar, print_progress_bar, ANSI_BLUE, ANSI_BOLD, ANSI_RESET, ANSI_YELLOW};
use crate::solution_runner::{build_timer, SolutionRunner, TestResult};
use crate::subtask::Subtask;
use crate::{Error, Input, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use crate::archiver::archive_files;
use crate::cpp_builder::build_solution;
use crate::partial_solution::run_partial_solution;

/// This struct represents an entire task.
/// You can add subtasks, partial solutions and set the time limit.
/// Once you are done, you can create tests for the task.
pub struct Task {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    // path to the folder with tests
    pub tests_path: PathBuf,
    // path to cpp file with solution
    pub solution_path: PathBuf,
    // time limit in seconds
    pub time_limit: f32,
    // path to the zip file with tests
    pub tests_archive_path: PathBuf,
    pub cps_tests_archive_path: PathBuf,
    // two closures that tells what should the input/output file be named for a given test
    // input to the closure is (test_id, subtask_id, test_id_in_subtask)
    pub get_input_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    pub get_output_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    pub(crate) solution_exe_path: PathBuf,
    pub(crate) build_folder_path: PathBuf,
    pub(crate) subtasks: Vec<Subtask>,
    pub(crate) partial_solutions: Vec<(PathBuf, HashSet<usize>)>,
}

impl Task {
    /// This function creates a new task with the given name and path.
    /// The path should be a relative path to the task folder in which the tests will be generated.
    /// The solution should be at `solution_path` which is `path`/solution.cpp by default but can be changed.
    #[must_use]
    pub fn new(name: &str, path: &Path) -> Self {
        let build_folder_path = path.join("build");
        Self {
            path: path.to_owned(),
            name: name.to_owned(),
            tests_path: path.join("tests"),
            solution_path: path.join("solution.cpp"),
            solution_exe_path: build_folder_path.join("solution"),
            tests_archive_path: path.join("tests.zip"),
            cps_tests_archive_path: path.join("tests.cpt"),
            get_input_file_name: Box::new(|test_id, subtask_id, _test_id_in_subtask| format!("test.{:02}.{:03}.in", subtask_id+1, test_id+1)),
            get_output_file_name: Box::new(|test_id, subtask_id, _test_id_in_subtask| format!("test.{:02}.{:03}.out", subtask_id+1, test_id+1)),
            build_folder_path,
            time_limit: 5.0,
            subtasks: Vec::new(),
            partial_solutions: Vec::new(),
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
        assert!(subtask < self.subtasks.len());
        assert!(dependency < subtask);
        self.subtasks[subtask].dependencies.push(dependency);
    }

    /// This function adds a partial solution to the task.
    /// A partial solution is a solution that only solves a subset of subtasks.
    /// When the task is generated, the partial solution will be run on all tests of the subtasks it solves.
    /// If the partial solution does not solve the exact subtasks it should, an error will be thrown.
    pub fn add_partial_solution(&mut self, solution_path: &str, passes_subtasks: &[usize]) {
        let set = passes_subtasks.iter().copied().collect::<HashSet<_>>();
        self.partial_solutions.push((self.path.join(solution_path), set));
    }

    /// This function does all the work.
    /// It builds the solution and all partial solutions, generates tests and checks them.
    pub fn create_tests(&mut self) -> Result<()> {
        self.create_tests_inner1(true, false)
    }

    /// This is the same as `create_tests` but it doesn't print anything.
    pub fn create_tests_no_print(&mut self) -> Result<()> {
        self.create_tests_inner1(false, false)
    }

    /// This also generates a CPS file.
    pub fn create_tests_for_cps(&mut self) -> Result<()> {
        self.create_tests_inner1(true, true)
    }

    /// This creates tests and prints the error message if there is an error.
    fn create_tests_inner1(&mut self, print_output: bool, generate_cps: bool) -> Result<()> {
        let logger = Logger::new(print_output);

        let start_time = std::time::Instant::now();
        let res = self.create_tests_inner2(&logger, generate_cps);
        if let Err(err) = res {
            logger.logln(format!("\n{ANSI_RED}{ANSI_BOLD}Error: {err}{ANSI_RESET}"));
            Err(err)
        } else {
            logger.logln(format!("\n{ANSI_GREEN}{ANSI_BOLD}Success!{ANSI_RESET}"));
            logger.logln(format!("Elapsed time: {ANSI_BOLD}{:.2}s{ANSI_RESET}\n", start_time.elapsed().as_secs_f32()));
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

    /// This function builds solution and then calls `generate_tests`.
    fn create_tests_inner2(&mut self, logger: &Logger, generate_cps: bool) -> Result<()> {
        logger.logln("");
        let text = format!("Creating tests for task \"{}\"", self.name);
        // print title with ===== before and after text
        logger.log(" ");
        for _ in 0..text.len() + 6 {
            logger.log("=");
        }
        logger.logln(format!("\n || {ANSI_BOLD}{text}{ANSI_RESET} ||"));
        logger.log(" ");
        for _ in 0..text.len() + 6 {
            logger.log("=");
        }
        logger.logln("\n");

        // if there are no subtasks, print a warning in bold yellow
        if self.subtasks.is_empty() {
            logger.logln(format!("{ANSI_YELLOW}{ANSI_BOLD}Warning: no subtasks{ANSI_RESET}"));
        }

        // create build directory if it doesn't exist
        if !self.build_folder_path.exists() {
            std::fs::create_dir_all(&self.build_folder_path).map_err(|err| Error::IOError { err, file: String::new() })?;
        }

        // check if solution file exists
        if !self.solution_path.exists() {
            return Err(Error::MissingSolutionFile { path: self.solution_path.to_str().unwrap_or("???").to_owned() });
        }

        // reset subtask input files
        for subtask in &mut self.subtasks {
            for test in &mut subtask.tests {
                test.reset_input_file();
            }
        }
        
        build_solution(&self.solution_path, &self.solution_exe_path, logger)?;

        for (i, partial_solution) in self.partial_solutions.iter().enumerate() {
            build_solution(&partial_solution.0, &self.build_folder_path.join(format!("partial_solution_{}", i + 1)), logger)?;
        }

        // create tests directory if it doesn't exist and clear it
        std::fs::create_dir_all(&self.tests_path).map_err(|err| Error::IOError { err, file: String::new() })?;
        for entry in std::fs::read_dir(&self.tests_path).map_err(|err| Error::IOError { err, file: String::new() })? {
            std::fs::remove_file(entry.map_err(|err| Error::IOError { err, file: String::new() })?.path()).map_err(|err| Error::IOError { err, file: String::new() })?;
        }
        
        let print_progress = |curr, total| {
            logger.log(format!("[{ANSI_BOLD}{curr}{ANSI_RESET}/{ANSI_BOLD}{total}{ANSI_RESET}] "));
        };
        
        build_timer(&self.build_folder_path, logger)?;

        print_progress(1,5);
        logger.logln(format!("{ANSI_BLUE}{ANSI_BOLD}Generating tests{ANSI_RESET}"));
        let test_files = self.generate_tests(logger)?;
        print_progress(2,5);
        logger.logln(format!("{ANSI_BLUE}{ANSI_BOLD}Checking generated tests{ANSI_RESET}"));
        self.check_tests(logger)?;
        print_progress(3,5);
        logger.logln(format!("{ANSI_BLUE}{ANSI_BOLD}Generating test solutions{ANSI_RESET}"));
        self.generate_test_solutions(logger, &test_files)?;
        print_progress(4,5);
        logger.logln(format!("{ANSI_BLUE}{ANSI_BOLD}Checking partial solutions{ANSI_RESET}"));
        self.check_partial_solutions(logger, &test_files)?;

        print_progress(5,5);
        if generate_cps {
            logger.logln(format!("{ANSI_BLUE}{ANSI_BOLD}Generating CPS file{ANSI_RESET}"));
            self.generate_cps_file()?;
        } else {
            logger.logln(format!("{ANSI_BLUE}{ANSI_BOLD}Archiving tests{ANSI_RESET}"));
            self.archive_tests(logger, &test_files)?;
        }

        let tests_size = fs_extra::dir::get_size(&self.tests_path).unwrap_or(0) as f32 / 1_000_000.0;
        logger.logln(format!("Tests size: {ANSI_BOLD}{tests_size:.2}MB{ANSI_RESET}"));

        Ok(())
    }
    
    fn generate_tests(&mut self, logger: &Logger) -> Result<Vec<Vec<(PathBuf, PathBuf)>>> {
        let num_tests = self.get_num_all_tests();

        // calculate how many steps there are in total for the progress bar.
        let loading_progress_max = num_tests;
        let mut loading_progress = 0;

        // Generate and write tests for each subtask
        let mut curr_test_id = 0;
        print_progress_bar(0.0, logger);
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
                    print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);
                    loading_progress += 1;
                }
            }

            test_files.push(tests_written);
        }

        clear_progress_bar(logger);
        
        assert_eq!(loading_progress, loading_progress_max);

        Ok(test_files)
    }
    
    fn check_tests(&self, logger: &Logger) -> Result<()> {
        // calculate how many steps there are in total for the progress bar.
        let mut loading_progress_max = 0;
        for subtask in &self.subtasks {
            if subtask.checker.is_some() {
                // and for each check
                loading_progress_max += self.get_total_tests(subtask);
            }
        }
        let mut loading_progress = 0;

        let mut curr_test_id = 0;
        print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);
        for (subtask_id, subtask) in self.subtasks.iter().enumerate() {
            let checker = &subtask.checker;
            if let Some(checker) = checker {
                for test_id_in_subtask in 0..self.get_total_tests(subtask) {
                    let input_test_path = self.get_input_file_path(curr_test_id, subtask_id as i32, test_id_in_subtask);
                    let input_str = std::fs::read_to_string(input_test_path).map_err(|err| Error::IOError { err, file: String::new() })?;
                    checker(Input::new(&input_str))?;
                    curr_test_id += 1;
                    loading_progress += 1;
                    print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);
                }
            } else {
                clear_progress_bar(logger);
                logger.logln(format!("{ANSI_YELLOW}{ANSI_BOLD}Warning{ANSI_RESET}: No checker for subtask {}.", subtask.number + 1));
                print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);
                curr_test_id += self.get_total_tests(subtask);
            }
        }
        clear_progress_bar(logger);

        assert_eq!(loading_progress, loading_progress_max);
        
        Ok(())
    }
    
    fn generate_test_solutions(&self, logger: &Logger, test_files: &Vec<Vec<(PathBuf, PathBuf)>>) -> Result<()> {
        let mut solution_runner = SolutionRunner::new();
        let mut test_tasks = Vec::new();
        
        // invoke solution on each test
        for subtask in test_files {
            let mut subtask_tasks = Vec::new();
            for (input_file, output_file) in subtask {
                
                subtask_tasks.push(solution_runner.add_task(self.solution_exe_path.clone(), input_file.clone(), output_file.clone(), self.time_limit));
            }
            test_tasks.push(subtask_tasks);
        }
        
        solution_runner.run_tasks(logger, &self.build_folder_path);

        let mut max_elapsed_time = 0;
        for (subtask, tasks) in test_files.iter().zip(test_tasks) {
            for ((input_file, _output_file), task) in subtask.iter().zip(tasks) {
                let test_result = solution_runner.get_result(task)?;
                let elapsed_time = match test_result {
                    TestResult::Ok(elapsed_time) => { elapsed_time }
                    TestResult::TimedOut => {
                        clear_progress_bar(logger);
                        return Err(Error::SolutionTimedOut {
                            test_path: input_file.to_str().unwrap_or("?").to_owned(),
                        });
                    }
                    TestResult::Crashed => {
                        clear_progress_bar(logger);
                        return Err(Error::SolutionFailed {
                            test_path: input_file.to_str().unwrap_or("?").to_owned(),
                        });
                    }
                };
                max_elapsed_time = max_elapsed_time.max(elapsed_time);
            }
        }
        logger.logln(format!("Solution time: {max_elapsed_time}ms"));
        
        Ok(())
    }
    
    fn check_partial_solutions(&self, logger: &Logger, test_files: &Vec<Vec<(PathBuf, PathBuf)>>) -> Result<()> {
        for (partial_id, partial_solution) in self.partial_solutions.iter().enumerate() {
            logger.logln(format!("Testing partial solution {}: {}", partial_id + 1, partial_solution.0.display()));

            let exe_path = self.build_folder_path.join(format!("partial_solution_{}", partial_id + 1));
            
            let subtasks_passed = run_partial_solution(test_files, &exe_path, logger, &self.build_folder_path, self.time_limit)?;

            for subtask_id in 0..self.subtasks.len() {
                let subtask_failed = !subtasks_passed.contains(&subtask_id);
                
                if subtask_failed && partial_solution.1.contains(&subtask_id) {
                    return Err(Error::PartialSolutionFailsSubtask {
                        partial_number: partial_id + 1,
                        subtask_number: subtask_id + 1,
                    });
                }

                if !subtask_failed && !partial_solution.1.contains(&subtask_id) {
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
    fn archive_tests(&self, logger: &Logger, test_files: &Vec<Vec<(PathBuf, PathBuf)>>) -> Result<()> {
        let mut test_files_vec = Vec::new();
        for subtask in test_files {
            for (input_file, output_file) in subtask {
                test_files_vec.push(input_file.clone());
                test_files_vec.push(output_file.clone());
            }
        }
        
        archive_files(&test_files_vec, &self.tests_archive_path, logger)?;

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
