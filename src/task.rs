use std::path::PathBuf;
use crate::subtask::Subtask;

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
    
    pub fn build(&mut self) {
        println!("Building task \"{}\"", self.name);
    }
}