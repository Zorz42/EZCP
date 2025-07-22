use crate::error::Result;
use crate::test::{Test, TestGenerator};
use crate::Input;
use std::rc::Rc;

/// This struct represents a subtask.
/// You can add tests, test generators and set a checker function.
/// Once you are done, you can add the subtask to a task.
pub struct Subtask {
    pub(super) number: usize,
    pub(super) tests: Vec<Test>,
    pub(super) dependencies: Vec<usize>,
    pub(super) checker: Option<Box<dyn Fn(Input) -> Result<()>>>,
}

impl Subtask {
    /// This function creates a new subtask with `points`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            number: 0,
            tests: Vec::new(),
            dependencies: Vec::new(),
            checker: None,
        }
    }

    /// This function adds a test generator to the subtask with the provided count.
    /// Test generator is a function that returns a string.
    pub fn add_test<F: Fn() -> String + 'static>(&mut self, number: i32, function: F) {
        let test_generator = Rc::new(TestGenerator::new(function));

        for _ in 0..(number as usize) {
            self.tests.push(Test::new(test_generator.clone()));
        }
    }

    /// This function adds a single test from a string.
    pub fn add_test_str<S: Into<String>>(&mut self, input: S) {
        let input: String = input.into();
        let func = move || input.clone();
        let test_generator = Rc::new(TestGenerator::new(func));
        self.tests.push(Test::new(test_generator));
    }

    /// This function sets the checker function for the subtask.
    /// The checker function receives the input and returns an error if the input is invalid.
    /// If the input is valid, it should return `Ok(())`.
    /// If the checker function is not set, the default checker will be used although it is not recommended.
    pub fn set_checker<F: Fn(Input) -> Result<()> + 'static>(&mut self, function: F) {
        self.checker = Some(Box::new(function));
    }
}
