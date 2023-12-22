use crate::test::{Test, TestGenerator};
use std::path::PathBuf;
use std::rc::Rc;

pub struct Subtask {
    pub(super) number: usize,
    pub(super) points: i32,
    pub(super) tests: Vec<Test>,
    pub(super) dependencies: Vec<usize>,
}

impl Subtask {
    pub fn new(points: i32) -> Subtask {
        Subtask {
            number: 0,
            points,
            tests: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    pub fn add_test<F>(&mut self, number: i32, function: F)
    where
        F: Fn() -> String + 'static,
    {
        let test_generator = Rc::new(TestGenerator::new(function));

        for _ in 0..(number as usize) {
            self.tests.push(Test::new(test_generator.clone()));
        }
    }

    pub fn add_test_str(&mut self, input: &str) {
        let input = input.to_owned();
        let func = move || input.clone();
        let test_generator = Rc::new(TestGenerator::new(func));
        self.tests.push(Test::new(test_generator));
    }

    pub(super) fn generate_tests(&mut self) {
        for test in &mut self.tests {
            test.generate_input();
        }
    }

    pub(super) fn write_tests(&self, curr_test_id: &mut i32, subtasks: &Vec<Subtask>, tests_path: &PathBuf, subtask_visited: &mut Vec<bool>) {
        if subtask_visited[self.number] {
            return;
        }
        subtask_visited[self.number] = true;

        for test in &self.tests {
            for dependency in &self.dependencies {
                subtasks[*dependency].write_tests(curr_test_id, subtasks, tests_path, subtask_visited);
            }

            let test_id = *curr_test_id;
            *curr_test_id += 1;
            let input_file_path = tests_path.join(format!("input.{:0>3}", test_id));
            std::fs::write(input_file_path, test.get_input()).unwrap();
        }
    }
}
