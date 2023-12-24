use crate::subtask::Subtask;
use crate::Input;
use anyhow::{anyhow, bail, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Task {
    name: String,
    path: PathBuf,
    pub tests_path: PathBuf,
    pub solution_path: PathBuf,
    solution_exe_path: PathBuf,
    build_folder_path: PathBuf,
    subtasks: Vec<Subtask>,
}

fn print_progress_bar(progress: f32) {
    let size = termsize::get();
    if let Some(size) = size {
        let bar_length = (size.cols as usize - 10).max(0);
        let num_filled = (progress * bar_length as f32) as usize;
        let num_empty = (bar_length - num_filled - 1).max(0);

        print!("\r {:.2}% [", progress * 100.0);
        for _ in 0..num_filled {
            print!("=");
        }
        if num_filled > 0 {
            print!(">");
        }
        for _ in 0..num_empty {
            print!(" ");
        }
        print!("]");
        std::io::stdout().flush().ok();
    }
}

fn clear_progress_bar() {
    let size = termsize::get();
    if let Some(size) = size {
        let bar_length = size.cols as usize;
        print!("\r");
        for _ in 0..bar_length {
            print!(" ");
        }
        print!("\r");
        std::io::stdout().flush().ok();
    }
}

impl Task {
    #[must_use]
    pub fn new(name: &str, path: &Path) -> Self {
        let build_folder_path = path.join("build");
        Self {
            name: name.to_owned(),
            path: path.to_path_buf(),
            tests_path: path.join("tests"),
            solution_path: path.join("solution.cpp"),
            solution_exe_path: build_folder_path.join("solution"),
            build_folder_path,
            subtasks: Vec::new(),
        }
    }

    pub fn add_subtask(&mut self, subtask: Subtask) -> usize {
        self.subtasks.push(subtask);
        self.subtasks.len() - 1
    }

    #[allow(clippy::indexing_slicing)]
    pub fn add_subtask_dependency(&mut self, subtask: usize, dependency: usize) {
        assert!(subtask < self.subtasks.len());
        assert!(dependency < subtask);
        self.subtasks[subtask].dependencies.push(dependency);
    }

    pub fn create_tests(&mut self) {
        let start_time = std::time::Instant::now();
        let res = self.create_tests_inner();
        if let Err(err) = res {
            println!("\x1b[31;1mError: {err}\x1b[0m");
        }
        println!("\x1b[32;1mElapsed time: {:.2}s\x1b[0m\n", start_time.elapsed().as_secs_f32());
    }

    pub fn create_tests_inner(&mut self) -> Result<()> {
        let text = format!("Creating tests for task \"{}\"", self.name);
        // print = before and after text
        for _ in 0..text.len() {
            print!("=");
        }
        println!("\n\x1b[1m{text}\x1b[0m");
        for _ in 0..text.len() {
            print!("=");
        }
        println!();

        // create task directory if it doesn't exist
        if !self.path.exists() {
            std::fs::create_dir_all(&self.path)?;
        }

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

        self.build_solution()?;

        self.generate_tests()?;

        Ok(())
    }

    fn generate_tests(&mut self) -> Result<()> {
        // create tests directory if it doesn't exist
        if !self.tests_path.exists() {
            std::fs::create_dir_all(&self.tests_path)?;
        }

        // delete all files in tests directory
        for entry in std::fs::read_dir(&self.tests_path)? {
            std::fs::remove_file(entry?.path())?;
        }

        let num_tests = {
            let mut result = 0;
            for subtask in &self.subtasks {
                result += self.get_total_tests(subtask)?;
            }
            result
        };

        let loading_progress_max = {
            let mut result = 2 * num_tests; // 2 generating input and producing output
            for subtask in &self.subtasks {
                if subtask.checker.is_some() {
                    // and for each check
                    result += self.get_total_tests(subtask)?;
                }
            }
            result
        };

        let tests_path = self.tests_path.clone();
        let mut curr_test_id = 0;
        println!("Generating tests...");
        print_progress_bar(0.0);
        for subtask_number in 0..self.subtasks.len() {
            let mut subtask_visited = vec![false; self.subtasks.len()];
            self.write_tests_for_subtask(subtask_number, &mut curr_test_id, &tests_path, &mut subtask_visited, loading_progress_max)?;
        }

        let mut loading_progress = num_tests;

        clear_progress_bar();
        println!("Checking tests...");
        print_progress_bar((loading_progress as f32) / (loading_progress_max as f32));
        curr_test_id = 0;
        for subtask in &self.subtasks {
            let checker = &subtask.checker;
            if let Some(checker) = checker {
                for _ in 0..self.get_total_tests(subtask)? {
                    let input_str = std::fs::read_to_string(self.tests_path.join(format!("input.{curr_test_id:0>3}")))?;
                    checker(Input::new(&input_str))?;
                    curr_test_id += 1;
                    loading_progress += 1;
                    print_progress_bar((loading_progress as f32) / (loading_progress_max as f32));
                }
            } else {
                clear_progress_bar();
                println!("\x1b[33mWarning: no checker for subtask {}\x1b[0m", subtask.number);
                print_progress_bar((loading_progress as f32) / (loading_progress_max as f32));
                curr_test_id += self.get_total_tests(subtask)?;
            }
        }

        clear_progress_bar();
        println!("Generating test solutions...");
        print_progress_bar((loading_progress as f32) / (loading_progress_max as f32));
        // invoke solution on each test
        let mut max_elapsed_time: f32 = 0.0;

        for test_id in 0..num_tests {
            print_progress_bar((loading_progress as f32) / (loading_progress_max as f32));

            let input_file_path = self.tests_path.join(format!("input.{test_id:0>3}"));
            let output_file_path = self.tests_path.join(format!("output.{test_id:0>3}"));

            // also time the solution
            let start_time = std::time::Instant::now();
            let mut solution_process = std::process::Command::new(&self.solution_exe_path)
                .stdin(std::fs::File::open(&input_file_path)?)
                .stdout(std::fs::File::create(&output_file_path)?)
                .spawn()?;

            let solution_status = solution_process.wait()?;
            let elapsed_time = start_time.elapsed().as_secs_f32();
            max_elapsed_time = max_elapsed_time.max(elapsed_time);
            loading_progress += 1;

            if !solution_status.success() {
                bail!("Solution failed on test {}", test_id);
            }
        }
        clear_progress_bar();
        let tests_size = fs_extra::dir::get_size(&self.tests_path)? as f32 / 1_000_000.0;

        println!("\x1b[32;1mMax solution time: {max_elapsed_time:.2}s, tests size: {tests_size:.2}MB\x1b[0m");

        Ok(())
    }

    fn write_tests_for_subtask(&mut self, subtask_number: usize, curr_test_id: &mut i32, tests_path: &PathBuf, subtask_visited: &mut Vec<bool>, loading_progress_max: i32) -> Result<()> {
        if *subtask_visited.get(subtask_number).ok_or_else(|| anyhow!("Subtask number out of bounds"))? {
            return Ok(());
        }
        *subtask_visited.get_mut(subtask_number).ok_or_else(|| anyhow!("Subtask number out of bounds"))? = true;

        let dependencies = self.subtasks[subtask_number].dependencies.clone();
        for dependency in dependencies {
            self.write_tests_for_subtask(dependency, curr_test_id, tests_path, subtask_visited, loading_progress_max)?;
        }

        for test in &mut self.subtasks[subtask_number].tests {
            let test_id = *curr_test_id;
            *curr_test_id += 1;
            let input_file_path = tests_path.join(format!("input.{test_id:0>3}"));
            test.generate_input(input_file_path.clone());
            print_progress_bar((*curr_test_id as f32) / (loading_progress_max as f32));
        }
        Ok(())
    }

    fn get_total_tests(&self, subtask: &Subtask) -> Result<i32> {
        let mut subtask_visited = vec![false; self.subtasks.len()];
        self.get_total_tests_inner(subtask.number, &mut subtask_visited)
    }

    fn get_total_tests_inner(&self, subtask_number: usize, subtask_visited: &mut Vec<bool>) -> Result<i32> {
        if *subtask_visited.get(subtask_number).ok_or_else(|| anyhow!("Subtask number out of bounds"))? {
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

    fn build_solution(&self) -> Result<()> {
        // if solution executable exists, check if it's up to date
        if self.solution_exe_path.exists() {
            let solution_last_modified = std::fs::metadata(&self.solution_path)?.modified()?;
            let solution_exe_last_modified = std::fs::metadata(&self.solution_exe_path)?.modified()?;

            if solution_exe_last_modified > solution_last_modified {
                println!("Skipping solution compilation as it is up to date");
                return Ok(());
            }
        }

        println!("Building solution...");

        // invoke g++ to build solution
        std::process::Command::new("g++")
            .arg("-std=c++17")
            .arg("-O2")
            .arg("-o")
            .arg(&self.solution_exe_path)
            .arg(&self.solution_path)
            .output()?;

        Ok(())
    }
}
