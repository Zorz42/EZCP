use crate::subtask::Subtask;
use std::path::PathBuf;

pub struct Task {
    name: String,
    path: PathBuf,
    tests_path: PathBuf,
    subtasks: Vec<Subtask>,
}

impl Task {
    pub fn new(name: &str, path: PathBuf) -> Task {
        Task {
            name: name.to_owned(),
            path: path.clone(),
            tests_path: path.join("tests"),
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

        // assign numbers to subtasks
        for (i, subtask) in self.subtasks.iter_mut().enumerate() {
            subtask.number = i;
        }

        println!("Generating tests...");
        self.create_tests();
        println!("Writing tests...");
        self.write_tests();
    }

    fn generate_tests(&mut self) {
        for subtask in &mut self.subtasks {
            subtask.generate_tests();
        }
    }

    fn write_tests(&self) {
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
    }
}
