use crate::subtask::Subtask;
use std::path::PathBuf;

pub struct Task {
    name: String,
    path: PathBuf,
    pub tests_path: PathBuf,
    pub solution_path: PathBuf,
    solution_exe_path: PathBuf,
    build_folder_path: PathBuf,
    subtasks: Vec<Subtask>,
}

impl Task {
    pub fn new(name: &str, path: PathBuf) -> Task {
        let build_folder_path = path.join("build");
        Task {
            name: name.to_owned(),
            path: path.clone(),
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

    pub fn add_subtask_dependency(&mut self, subtask: usize, dependency: usize) {
        self.subtasks[subtask].dependencies.push(dependency);
    }

    pub fn create_tests(&mut self) {
        println!("Creating tests for task \"{}\"", self.name);

        // create task directory if it doesn't exist
        if !self.path.exists() {
            std::fs::create_dir(&self.path).unwrap();
        }

        // create build directory if it doesn't exist
        if !self.build_folder_path.exists() {
            std::fs::create_dir(&self.build_folder_path).unwrap();
        }

        // check if solution file exists
        if !self.solution_path.exists() {
            panic!("Solution file \"{}\" doesn't exist", self.solution_path.to_str().unwrap());
        }

        // assign numbers to subtasks
        for (i, subtask) in self.subtasks.iter_mut().enumerate() {
            subtask.number = i;
        }

        self.generate_tests();
        let num_tests = self.write_tests();
        self.build_solution();
        self.generate_test_solutions(num_tests);
    }

    fn generate_tests(&mut self) {
        println!("Generating tests...");
        for subtask in &mut self.subtasks {
            subtask.generate_tests();
        }
    }

    fn write_tests(&self) -> i32 {
        println!("Writing tests...");

        // create tests directory if it doesn't exist
        if !self.tests_path.exists() {
            std::fs::create_dir(&self.tests_path).unwrap();
        }

        // delete all files in tests directory
        for entry in std::fs::read_dir(&self.tests_path).unwrap() {
            let entry = entry.unwrap();
            std::fs::remove_file(entry.path()).unwrap();
        }

        let mut curr_test_id = 0;
        for subtask in &self.subtasks {
            let mut subtask_visited = vec![false; self.subtasks.len()];
            subtask.write_tests(&mut curr_test_id, &self.subtasks, &self.tests_path, &mut subtask_visited);
        }

        curr_test_id
    }

    fn build_solution(&self) {
        // if solution executable exists, check if it's up to date
        if self.solution_exe_path.exists() {
            let solution_last_modified = std::fs::metadata(&self.solution_path).unwrap().modified().unwrap();
            let solution_exe_last_modified = std::fs::metadata(&self.solution_exe_path).unwrap().modified().unwrap();

            if solution_exe_last_modified > solution_last_modified {
                println!("Skipping solution compilation as it is up to date");
                return;
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
            .output()
            .expect("Failed to build solution");
    }

    fn generate_test_solutions(&self, num_tests: i32) {
        println!("Generating test solutions...");
        // invoke solution on each test
        for test_id in 0..num_tests {
            let input_file_path = self.tests_path.join(format!("input.{:0>3}", test_id));
            let output_file_path = self.tests_path.join(format!("output.{:0>3}", test_id));

            let mut solution_process = std::process::Command::new(&self.solution_exe_path)
                .stdin(std::fs::File::open(&input_file_path).unwrap())
                .stdout(std::fs::File::create(&output_file_path).unwrap())
                .spawn()
                .expect("Failed to run solution");

            let solution_status = solution_process.wait().expect("Failed to wait for solution");

            if !solution_status.success() {
                panic!("Solution failed on test {}", test_id);
            }
        }
    }
}
