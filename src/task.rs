use crate::logger::Logger;
use crate::solution_runner::{are_files_equal, build_solution, run_solution};
use crate::subtask::Subtask;
use crate::Input;
use anyhow::{anyhow, bail, Result};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

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
    // two closures that tells what should the input/output file be named for a given test
    // input to the closure is (test_id, subtask_id, test_id_in_subtask)
    pub get_input_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    pub get_output_file_name: Box<dyn Fn(i32, i32, i32) -> String>,
    solution_exe_path: PathBuf,
    build_folder_path: PathBuf,
    subtasks: Vec<Subtask>,
    partial_solutions: Vec<(PathBuf, HashSet<usize>)>,
}

fn print_progress_bar(progress: f32, logger: &Logger) {
    let size = termsize::get();
    logger.log(format!("\r {:.2}% [", progress * 100.0));

    let bar_length = size.map_or(10, |size| (size.cols as usize - 10).max(0));
    let num_filled = (progress * bar_length as f32) as usize;
    let num_empty = (bar_length - num_filled - 1).max(0);

    for _ in 0..num_filled {
        logger.log("=");
    }
    if num_filled > 0 {
        logger.log(">");
    }
    for _ in 0..num_empty {
        logger.log(" ");
    }
    logger.log("]");

    std::io::stdout().flush().ok();
}

fn clear_progress_bar(logger: &Logger) {
    let size = termsize::get();
    let bar_length = size.map_or(10, |size| size.cols as usize);

    logger.log("\r");
    for _ in 0..bar_length {
        logger.log(" ");
    }
    logger.log("\r");
    std::io::stdout().flush().ok();
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
            get_input_file_name: Box::new(|test_id, _subtask_id, _test_id_in_subtask| format!("input.{test_id:0>3}")),
            get_output_file_name: Box::new(|test_id, _subtask_id, _test_id_in_subtask| format!("output.{test_id:0>3}")),
            build_folder_path,
            time_limit: 1.0,
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
    pub fn add_subtask(&mut self, subtask: Subtask) -> usize {
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
    pub fn create_tests(&mut self) -> bool {
        self.create_tests_inner1(true, false)
    }

    /// This is the same as `create_tests` but it doesn't print anything.
    pub fn create_tests_no_print(&mut self) -> bool {
        self.create_tests_inner1(false, false)
    }

    pub fn create_tests_for_cps(&mut self) -> bool {
        self.create_tests_inner1(true, true)
    }

    fn create_tests_inner1(&mut self, print_output: bool, generate_cps: bool) -> bool {
        let logger = Logger::new(print_output);

        let start_time = std::time::Instant::now();
        let res = self.create_tests_inner2(&logger, generate_cps);
        let is_ok = res.is_ok();
        if let Err(err) = res {
            logger.logln(format!("\n\x1b[31;1mError: {err}\x1b[0m"));
            // print backtrace if not in release mode
            if cfg!(debug_assertions) {
                logger.logln(format!("\x1b[31;1mBacktrace: {backtrace}\x1b[0m", backtrace = err.backtrace()));
            }
        } else {
            logger.logln("\n\x1b[32;1mSuccess!\x1b[0m");
        }
        logger.logln(format!("\x1b[36;1mElapsed time: {:.2}s\n\x1b[0m", start_time.elapsed().as_secs_f32()));
        is_ok
    }

    fn create_tests_inner2(&mut self, logger: &Logger, generate_cps: bool) -> Result<()> {
        logger.logln("");
        let text = format!("Creating tests for task \"{}\"", self.name);
        // print = before and after text
        for _ in 0..text.len() {
            logger.log("=");
        }
        logger.logln(format!("\n\x1b[1m{text}\x1b[0m"));
        for _ in 0..text.len() {
            logger.log("=");
        }
        logger.logln("");

        // create build directory if it doesn't exist
        if !self.build_folder_path.exists() {
            std::fs::create_dir_all(&self.build_folder_path)?;
        }

        // check if solution file exists
        if !self.solution_path.exists() {
            bail!("Solution file \"{}\" doesn't exist", self.solution_path.to_str().unwrap_or("path error"));
        }

        // assign numbers to subtasks
        for (i, subtask) in self.subtasks.iter_mut().enumerate() {
            subtask.number = i;
        }

        // reset subtask input files
        for subtask in &mut self.subtasks {
            for test in &mut subtask.tests {
                test.reset_input_file();
            }
        }

        logger.logln("Building solution...");
        let has_built = build_solution(&self.solution_path, &self.solution_exe_path)?;
        if !has_built {
            logger.logln("Skipping solution compilation as it is up to date.");
        }

        for (i, partial_solution) in self.partial_solutions.iter().enumerate() {
            logger.logln("Building partial solution...");
            let has_built = build_solution(&partial_solution.0, &self.build_folder_path.join(format!("partial_solution_{i}")))?;
            if !has_built {
                logger.logln(format!("Skipping partial solution {i} compilation as it is up to date."));
            }
        }

        self.generate_tests(logger, generate_cps)?;

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn generate_tests(&mut self, logger: &Logger, generate_cps: bool) -> Result<()> {
        // create tests directory if it doesn't exist and clear it
        std::fs::create_dir_all(&self.tests_path)?;
        for entry in std::fs::read_dir(&self.tests_path)? {
            std::fs::remove_file(entry?.path())?;
        }

        // count how many tests there are in total (if one subtask is a dependency of another, its tests are counted twice)
        let num_tests = {
            let mut result = 0;
            for subtask in &self.subtasks {
                result += self.get_total_tests(subtask)?;
            }
            result
        };

        // calculate how many steps there are in total for the progress bar. If checkers are missing, it is less steps.
        let loading_progress_max = {
            let mut result = 2 * num_tests + self.partial_solutions.len() as i32 * num_tests; // 2 generating input and producing output and num_tests for every partial solution
            for subtask in &self.subtasks {
                if subtask.checker.is_some() {
                    // and for each check
                    result += self.get_total_tests(subtask)?;
                }
            }
            result
        };

        logger.logln("Generating tests...");

        // Generate and write tests for each subtask
        let mut curr_test_id = 0;
        print_progress_bar(0.0, logger);
        let mut test_files = Vec::new();
        for subtask_number in 0..self.subtasks.len() {
            let mut curr_local_test_id = 0;
            let mut subtask_visited = vec![false; self.subtasks.len()];
            let mut tests_written = Vec::new();
            self.write_tests_for_subtask(
                subtask_number,
                subtask_number,
                &mut curr_test_id,
                &mut curr_local_test_id,
                &mut subtask_visited,
                loading_progress_max,
                logger,
                &mut tests_written,
            )?;
            test_files.push(tests_written);
        }

        // loading progress at this point is exactly num_tests
        let mut loading_progress = num_tests;

        clear_progress_bar(logger);
        logger.logln("Checking tests...");
        print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);

        // check all tests
        curr_test_id = 0;
        for (subtask_id, subtask) in self.subtasks.iter().enumerate() {
            let checker = &subtask.checker;
            if let Some(checker) = checker {
                for test_id_in_subtask in 0..self.get_total_tests(subtask)? {
                    let input_str = std::fs::read_to_string(self.get_input_file_path(curr_test_id, subtask_id as i32, test_id_in_subtask))?;
                    checker(Input::new(&input_str))?;
                    curr_test_id += 1;
                    loading_progress += 1;
                    print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);
                }
            } else {
                clear_progress_bar(logger);
                logger.logln(format!("\x1b[33mWarning: no checker for subtask {}\x1b[0m", subtask.number));
                print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);
                curr_test_id += self.get_total_tests(subtask)?;
            }
        }

        clear_progress_bar(logger);
        logger.logln("Generating test solutions...");
        print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);

        // invoke solution on each test
        let mut max_elapsed_time: f32 = 0.0;
        let mut curr_test_id = 0;
        for subtask in &test_files {
            for (input_file, output_file) in subtask {
                print_progress_bar((loading_progress as f32) / (loading_progress_max as f32), logger);

                loading_progress += 1;
                let elapsed_time = run_solution(&self.solution_exe_path, input_file, output_file, self.time_limit, curr_test_id)?;
                curr_test_id += 1;
                max_elapsed_time = max_elapsed_time.max(elapsed_time);
            }
        }
        clear_progress_bar(logger);
        let tests_size = fs_extra::dir::get_size(&self.tests_path)? as f32 / 1_000_000.0;

        for (partial_id, partial_solution) in self.partial_solutions.iter().enumerate() {
            logger.logln(format!("Checking partial solution {partial_id}..."));

            let mut passed_subtasks = HashSet::new();

            let mut curr_test_id = 0;
            for (subtask, subtask_tests) in self.subtasks.iter().zip(&test_files) {
                let mut subtask_failed = false;
                for (input_file, output_file) in subtask_tests {
                    if !subtask_failed {
                        let exe_path = self.build_folder_path.join(format!("partial_solution_{partial_id}"));
                        let temp_output_file = self.build_folder_path.join("temp_output");

                        let result = run_solution(&exe_path, input_file, &temp_output_file, self.time_limit, curr_test_id);

                        if result.is_err() {
                            subtask_failed = true;
                            continue;
                        }

                        if !are_files_equal(&temp_output_file, output_file)? {
                            subtask_failed = true;
                            continue;
                        }
                    }

                    curr_test_id += 1;
                }

                if !subtask_failed {
                    passed_subtasks.insert(subtask.number);
                }
            }

            for should_pass in &partial_solution.1 {
                if !passed_subtasks.contains(should_pass) {
                    bail!("Partial solution {partial_id} doesn't pass subtask {should_pass}");
                }
            }

            for has_passed in &passed_subtasks {
                if !partial_solution.1.contains(has_passed) {
                    bail!("Partial solution {partial_id} passes subtask {has_passed} which it shouldn't");
                }
            }
        }

        if generate_cps {
            println!("Generating CPS file...");
            self.generate_cps_file()?;
        } else {
            println!("Archiving tests...");
            self.archive_tests(&test_files)?;
        }

        logger.logln(format!("\x1b[36;1mMax solution time: {max_elapsed_time:.2}s, tests size: {tests_size:.2}MB\x1b[0m"));

        Ok(())
    }

    fn write_tests_for_subtask(
        &mut self,
        subtask_number: usize,
        master_subtask: usize,
        curr_test_id: &mut i32,
        curr_local_test_id: &mut i32,
        subtask_visited: &mut Vec<bool>,
        loading_progress_max: i32,
        logger: &Logger,
        tests_written: &mut Vec<(PathBuf, PathBuf)>,
    ) -> Result<()> {
        // check if subtask has already been visited
        if subtask_visited[subtask_number] {
            return Ok(());
        }
        subtask_visited[subtask_number] = true;

        // first, write tests for dependencies
        let dependencies = self.subtasks[subtask_number].dependencies.clone();
        for dependency in dependencies {
            self.write_tests_for_subtask(
                dependency,
                master_subtask,
                curr_test_id,
                curr_local_test_id,
                subtask_visited,
                loading_progress_max,
                logger,
                tests_written,
            )?;
        }

        // generate input files paths for all tests because of rust borrow checker
        let mut tests_input_files = Vec::new();
        let num_tests = self.subtasks[subtask_number].tests.len();
        let initial_progress = *curr_test_id;
        for _ in 0..num_tests {
            let input_path = self.get_input_file_path(*curr_test_id, master_subtask as i32, *curr_local_test_id);
            let output_path = self.get_output_file_path(*curr_test_id, master_subtask as i32, *curr_local_test_id);
            tests_input_files.push(input_path.clone());
            tests_written.push((input_path, output_path));
            *curr_test_id += 1;
            *curr_local_test_id += 1;
        }

        // generate input files for all tests
        let mut progress = initial_progress;
        for (test, input_file) in &mut self.subtasks[subtask_number].tests.iter_mut().zip(tests_input_files) {
            progress += 1;
            test.generate_input(&input_file)?;
            print_progress_bar((progress as f32) / (loading_progress_max as f32), logger);
        }
        Ok(())
    }

    fn get_total_tests(&self, subtask: &Subtask) -> Result<i32> {
        let mut subtask_visited = vec![false; self.subtasks.len()];
        self.get_total_tests_inner(subtask.number, &mut subtask_visited)
    }

    fn get_total_tests_inner(&self, subtask_number: usize, subtask_visited: &mut Vec<bool>) -> Result<i32> {
        // check if subtask has already been visited
        if subtask_visited[subtask_number] {
            return Ok(0);
        }
        *subtask_visited.get_mut(subtask_number).ok_or_else(|| anyhow!("Subtask number out of bounds"))? = true;

        let mut result = 0;
        for dependency in &self.subtasks[subtask_number].dependencies {
            result += self.get_total_tests_inner(*dependency, subtask_visited)?;
        }

        result += self.subtasks[subtask_number].tests.len() as i32;

        Ok(result)
    }

    fn archive_tests(&self, test_files: &Vec<Vec<(PathBuf, PathBuf)>>) -> Result<()> {
        let mut zipper = zip::ZipWriter::new(std::fs::File::create(&self.tests_archive_path)?);
        let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        for subtask in test_files {
            for (input_file, output_file) in subtask {
                zipper.start_file(input_file.file_name().map_or("", |a| a.to_str().unwrap_or("")), options)?;
                zipper.write_all(&std::fs::read(input_file)?)?;

                zipper.start_file(output_file.file_name().map_or("", |a| a.to_str().unwrap_or("")), options)?;
                zipper.write_all(&std::fs::read(output_file)?)?;
            }
        }

        Ok(())
    }

    fn generate_cps_file(&self) -> Result<()> {
        /*let mut cps_tests = CPSTests {
            tests: Vec::new(),
            subtask_tests: Vec::new(),
            subtask_points: Vec::new(),
        };

        for subtask in &self.subtasks {
            let mut subtask_tests = Vec::new();
            for dependency in &subtask.dependencies {
                subtask_tests.extend_from_slice(&cps_tests.subtask_tests[*dependency]);
            }
            for test in &subtask.tests {
                subtask_tests.push(cps_tests.tests.len());
            }
        }*/

        Ok(())
    }
}
