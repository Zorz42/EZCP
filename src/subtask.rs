use crate::test::{Test, TestGenerator};
use crate::Input;
use anyhow::Result;
use std::rc::Rc;

pub struct Subtask {
    pub(super) number: usize,
    pub(super) _points: i32,
    pub(super) tests: Vec<Test>,
    pub(super) dependencies: Vec<usize>,
    pub(super) checker: Option<Box<dyn Fn(Input) -> Result<()>>>,
}

impl Subtask {
    #[must_use]
    pub fn new(points: i32) -> Self {
        Self {
            number: 0,
            _points: points,
            tests: Vec::new(),
            dependencies: Vec::new(),
            checker: None,
        }
    }

    pub fn add_test<F: Fn() -> String + 'static>(&mut self, number: i32, function: F) {
        let test_generator = Rc::new(TestGenerator::new(function));

        for _ in 0..(number as usize) {
            self.tests.push(Test::new(test_generator.clone()));
        }
    }

    pub fn add_test_str<S: Into<String>>(&mut self, input: S) {
        let input: String = input.into();
        let func = move || input.clone();
        let test_generator = Rc::new(TestGenerator::new(func));
        self.tests.push(Test::new(test_generator));
    }

    pub fn set_checker<F: Fn(Input) -> Result<()> + 'static>(&mut self, function: F) {
        self.checker = Some(Box::new(function));
    }
}
