use std::rc::Rc;
use crate::test::{Test, TestGenerator};

pub struct Subtask {
    points: i32,
    tests: Vec<Test>,
}

impl Subtask {
    pub fn new(points: i32) -> Subtask {
        Subtask {
            points,
            tests: Vec::new(),
        }
    }
    
    pub fn add_test<F>(&mut self, number: i32, function: F) where F: Fn() -> String + 'static {
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
    
    pub fn add_dependency(&mut self, subtask: &Subtask) {
        for test in &subtask.tests {
            self.tests.push(test.clone());
        }
    }
}