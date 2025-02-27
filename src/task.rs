use crate::logger::Logger;
use crate::progress_bar::{clear_progress_bar, print_progress_bar};
use crate::solution_runner::{are_files_equal, build_solution, run_solution, TestResult};
use crate::subtask::Subtask;
use crate::{Error, Input, Result};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

#[derive(serde::Serialize)]
pub struct CPSTests {
    pub tests: Vec<(String, String)>,
    pub subtask_tests: Vec<Vec<usize>>,
    pub subtask_points: Vec<i32>,
}

/// This struct represents an entire task.
/// You can add subtasks, partial solutions and set the time limit.
/// Once you are done, you can create tests for the task.
pub struct Task {
    name: String,
    path: PathBuf,
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
    solution_exe_path: PathBuf,
    build_folder_path: PathBuf,
    subtasks: Vec<Subtask>,
    partial_solutions: Vec<(PathBuf, HashSet<usize>)>,
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
            get_input_file_name: Box::new(|test_id, _subtask_id, _test_id_in_subtask| format!("test{test_id:0>3}.in")),
            get_output_file_name: Box::new(|test_id, _subtask_id, _test_id_in_subtask| format!("test{test_id:0>3}.out")),
            build_folder_path,
            time_limit: 5.0,
            subtasks: Vec::new(),
            partial_solutions: Vec::new(),
        }
    }

    fn get_input_file_path(&self, test_id: i32, subtask_id: i32, test_id_in_subtask: i32) -> PathBuf {
        self.tests_path.join((self.get_input_file_name)(test_id, subtask_id, test_id_in_subtask))
    }

    fn get_output_file_path(&self, test_id: i32, subtask_id: i32, test_id_in_subtask: i32) -> PathBuf {
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
            logger.logln(format!("\n\x1b[31;1mError: {err}\x1b[0m"));
            Err(err)
        } else {
            logger.logln("\n\x1b[32;1mSuccess!\x1b[0m");
            logger.logln(format!("\x1b[36;1mElapsed time: {:.2}s\n\x1b[0m", start_time.elapsed().as_secs_f32()));
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
        logger.logln(format!("\n || \x1b[1m{text}\x1b[0m ||"));
        logger.log(" ");
        for _ in 0..text.len() + 6 {
            logger.log("=");
        }
        logger.logln("\n");

        // if there are no subtasks, print a warning in bold yellow
        if self.subtasks.is_empty() {
            logger.logln("\x1b[33;1mWarning: no subtasks\x1b[0m");
        }

        // create build directory if it doesn't exist
        if !self.build_folder_path.exists() {
            std::fs::create_dir_all(&self.build_folder_path).map_err(|err| Error::IOError { err })?;
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
        
        let has_built = build_solution(&self.solution_path, &self.solution_exe_path)?;
        if has_built {
            logger.logln(format!("Built solution {:?}", self.solution_path));
        }

        for (i, partial_solution) in self.partial_solutions.iter().enumerate() {
            let has_built = build_solution(&partial_solution.0, &self.build_folder_path.join(format!("partial_solution_{}", i + 1)))?;
            if has_built {
                logger.logln(format!("Built partial solution {:?}", partial_solution.0));
            }
        }

        // create tests directory if it doesn't exist and clear it
        std::fs::create_dir_all(&self.tests_path).map_err(|err| Error::IOError { err })?;
        for entry in std::fs::read_dir(&self.tests_path).map_err(|err| Error::IOError { err })? {
            std::fs::remove_file(entry.map_err(|err| Error::IOError { err })?.path()).map_err(|err| Error::IOError { err })?;
        }

        logger.logln("[1/5] Generating tests...");
        let test_files = self.generate_tests(logger)?;
        logger.logln("[2/5] Checking tests...");
        self.check_tests(logger)?;
        logger.logln("[3/5] Generating test solutions...");
        self.generate_test_solutions(logger, &test_files)?;
        logger.logln("[4/5] Checking partial solutions ...");
        self.check_partial_solutions(logger, &test_files)?;

        if generate_cps {
            println!("[5/5] Generating CPS file...");
            self.generate_cps_file()?;
        } else {
            println!("[5/5] Archiving tests...");
            self.archive_tests(logger, &test_files)?;
        }

        let tests_size = fs_extra::dir::get_size(&self.tests_path).unwrap_or(0) as f32 / 1_000_000.0;
        logger.logln(format!("\x1b[36;1mTests size: {tests_size:.2}MB\x1b[0m"));

        Ok(())
    }
    
    #[allow(clippy::too_many_lines)]
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
                    test.generate_input(&input_file)?;
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
                    let input_str = std::fs::read_to_string(input_test_path).map_err(|err| Error::IOError { err })?;
                    checker(Input::new(&input_str))?;
                    curr_test_id += 1;
                    loading_progress += 1;
                    print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);
                }
            } else {
                clear_progress_bar(logger);
                logger.logln(format!("\x1b[33mWarning: no checker for subtask {}\x1b[0m", subtask.number));
                print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);
                curr_test_id += self.get_total_tests(subtask);
            }
        }
        clear_progress_bar(logger);

        assert_eq!(loading_progress, loading_progress_max);
        
        Ok(())
    }
    
    fn generate_test_solutions(&self, logger: &Logger, test_files: &Vec<Vec<(PathBuf, PathBuf)>>) -> Result<()> {
        let num_tests = self.get_num_all_tests();
        
        // calculate how many steps there are in total for the progress bar.
        let loading_progress_max = num_tests;
        let mut loading_progress = 0;
        
        print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);

        // invoke solution on each test
        let mut max_elapsed_time: f32 = 0.0;
        for subtask in test_files {
            for (input_file, output_file) in subtask {
                print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);
                loading_progress += 1;
                
                let test_result = run_solution(&self.solution_exe_path, input_file, output_file, self.time_limit)?;
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
                            test_path: input_file.to_str().unwrap_or("???").to_owned(),
                        });
                    }
                };
                max_elapsed_time = max_elapsed_time.max(elapsed_time);
            }
        }
        clear_progress_bar(logger);

        assert_eq!(loading_progress, loading_progress_max);
        
        Ok(())
    }
    
    fn check_partial_solutions(&self, logger: &Logger, test_files: &Vec<Vec<(PathBuf, PathBuf)>>) -> Result<()> {
        let num_tests = self.get_num_all_tests();
        
        // calculate how many steps there are in total for the progress bar.
        let loading_progress_max = self.partial_solutions.len() as i32 * num_tests;
        let mut loading_progress = 0;
        
        for (partial_id, partial_solution) in self.partial_solutions.iter().enumerate() {
            logger.logln(format!("Checking partial solution {}...", partial_id + 1));
            
            print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);
            for (subtask_id, (_subtask, subtask_tests)) in self.subtasks.iter().zip(test_files).enumerate() {
                let mut subtask_failed = false;
                let mut err_message = String::new();
                for (input_file, output_file) in subtask_tests {
                    if !subtask_failed {
                        let exe_path = self.build_folder_path.join(format!("partial_solution_{}", partial_id + 1));
                        let temp_output_file = self.build_folder_path.join("temp_output");

                        let result = run_solution(&exe_path, input_file, &temp_output_file, self.time_limit)?;

                        match result {
                            TestResult::Ok(_) => {}
                            TestResult::TimedOut => {
                                err_message = "Partial solution timed out".to_owned();
                                subtask_failed = true;
                            }
                            TestResult::Crashed => {
                                err_message = "Partial solution crashed".to_owned();
                                subtask_failed = true;
                            }
                        }

                        if !are_files_equal(&temp_output_file, output_file)? {
                            err_message = "Wrong Answer".to_owned();
                            subtask_failed = true;
                        }
                    }

                    print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);
                    loading_progress += 1;
                }

                if subtask_failed && partial_solution.1.contains(&subtask_id) {
                    clear_progress_bar(logger);
                    return Err(Error::PartialSolutionFailsSubtask {
                        partial_number: partial_id + 1,
                        subtask_number: subtask_id + 1,
                        message: err_message,
                    });
                }

                if !subtask_failed && !partial_solution.1.contains(&subtask_id) {
                    clear_progress_bar(logger);
                    return Err(Error::PartialSolutionPassesExtraSubtask {
                        partial_number: partial_id + 1,
                        subtask_number: subtask_id + 1,
                    });
                }
            }

            clear_progress_bar(logger);
        }
        
        assert_eq!(loading_progress, loading_progress_max);
        
        Ok(())
    }
    
    /// Archive all tests into a zip file
    fn archive_tests(&self, logger: &Logger, test_files: &Vec<Vec<(PathBuf, PathBuf)>>) -> Result<()> {
        let mut zipper = zip::ZipWriter::new(std::fs::File::create(&self.tests_archive_path).map_err(|err| Error::IOError { err })?);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        let loading_progress_max = test_files.iter().map(Vec::len).sum::<usize>() as i32;
        let mut loading_progress = 0;
        
        for subtask in test_files {
            for (input_file, output_file) in subtask {
                print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);
                loading_progress += 1;
                
                zipper
                    .start_file(input_file.file_name().map_or("", |a| a.to_str().unwrap_or("")), options)
                    .map_err(|err| Error::ZipError { err })?;
                let input_file = std::fs::read(input_file).map_err(|err| Error::IOError { err })?;
                zipper.write_all(&input_file).map_err(|err| Error::IOError { err })?;

                zipper
                    .start_file(output_file.file_name().map_or("", |a| a.to_str().unwrap_or("")), options)
                    .map_err(|err| Error::ZipError { err })?;
                let output_file = std::fs::read(output_file).map_err(|err| Error::IOError { err })?;
                zipper.write_all(&output_file).map_err(|err| Error::IOError { err })?;
            }
        }
        
        clear_progress_bar(logger);

        Ok(())
    }
    
    /// Generate a CPS compatible
    fn generate_cps_file(&self) -> Result<()> {
        let mut cps_tests = CPSTests {
            tests: Vec::new(),
            subtask_tests: vec![Vec::new(); self.subtasks.len()],
            subtask_points: vec![0; self.subtasks.len()],
        };

        for subtask in &self.subtasks {
            cps_tests.subtask_points[subtask.number] = subtask.points;

            let mut subtask_tests = Vec::new();
            for dependency in &subtask.dependencies {
                subtask_tests.extend_from_slice(&cps_tests.subtask_tests[*dependency]);
            }
            for _test in &subtask.tests {
                let input_file = self.get_input_file_path(cps_tests.tests.len() as i32, subtask.number as i32, subtask_tests.len() as i32);
                let output_file = self.get_output_file_path(cps_tests.tests.len() as i32, subtask.number as i32, subtask_tests.len() as i32);

                let input = std::fs::read_to_string(&input_file).map_err(|err| Error::IOError { err })?;
                let output = std::fs::read_to_string(&output_file).map_err(|err| Error::IOError { err })?;

                subtask_tests.push(cps_tests.tests.len());

                cps_tests.tests.push((input, output));
            }
            cps_tests.subtask_tests[subtask.number] = subtask_tests;
        }

        let mut buffer = Vec::new();
        bincode::serialize_into(&mut buffer, &cps_tests).map_err(|err| Error::BincodeError { err })?;
        let data = snap::raw::Encoder::new().compress_vec(&buffer).map_err(|err| Error::SnapError { err })?;
        std::fs::write(&self.cps_tests_archive_path, data).map_err(|err| Error::IOError { err })?;

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
