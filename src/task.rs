use crate::subtask::Subtask;
use anyhow::{bail, Result};
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
        assert!(dependency < self.subtasks.len());
        self.subtasks[subtask].dependencies.push(dependency);
    }

    pub fn create_tests(&mut self) {
        let res = self.create_tests_inner();
        if let Err(err) = res {
            println!("\x1b[31;1mError: {err}\x1b[0m");
        }
        println!();
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

        self.generate_tests();
        let num_tests = self.write_tests()?;
        self.build_solution()?;
        self.generate_test_solutions(num_tests)?;

        Ok(())
    }

    fn generate_tests(&mut self) {
        println!("Generating tests...");
        for subtask in &mut self.subtasks {
            subtask.generate_tests();
        }
    }

    fn write_tests(&self) -> Result<i32> {
        println!("Writing tests...");

        // create tests directory if it doesn't exist
        if !self.tests_path.exists() {
            std::fs::create_dir_all(&self.tests_path)?;
        }

        // delete all files in tests directory
        for entry in std::fs::read_dir(&self.tests_path)? {
            std::fs::remove_file(entry?.path())?;
        }

        let mut curr_test_id = 0;
        for subtask in &self.subtasks {
            let mut subtask_visited = vec![false; self.subtasks.len()];
            let checker = subtask.checker.as_deref();
            if checker.is_none() {
                println!("\x1b[33mWarning: no checker for subtask {}\x1b[0m", subtask.number);
            }
            println!("Writing subtask {}...", subtask.number);
            subtask.write_tests(&mut curr_test_id, &self.subtasks, &self.tests_path, &mut subtask_visited, checker)?;
        }

        Ok(curr_test_id)
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

    fn generate_test_solutions(&self, num_tests: i32) -> Result<()> {
        println!("Generating test solutions...");
        // invoke solution on each test
        let mut max_elapsed_time: f32 = 0.0;

        for test_id in 0..num_tests {
            print_progress_bar((test_id as f32) / (num_tests as f32));

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

            if !solution_status.success() {
                bail!("Solution failed on test {}", test_id);
            }
        }
        clear_progress_bar();
        let tests_size = fs_extra::dir::get_size(&self.tests_path)? as f32 / 1_000_000.0;

        println!("\x1b[32;1mMax solution time: {max_elapsed_time:.2}s, tests size: {tests_size:.2}MB\x1b[0m");

        Ok(())
    }
}
