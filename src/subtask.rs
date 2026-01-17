use crate::test::TestGenerator;
use std::rc::Rc;

/// This struct represents a subtask.
/// You can add generators that produce test inputs.
/// Once you are done, you can add the subtask to a task.
/// 
/// Tests are generated dynamically during task creation using the registered generators.
pub struct Subtask {
    pub(super) number: usize,
    /// Generators that produce test inputs
    pub(super) generators: Vec<Rc<TestGenerator>>,
    /// How many tests to generate from each generator initially
    pub(super) initial_counts: Vec<usize>,
}

impl Subtask {
    /// This function creates a new subtask.
    #[must_use]
    pub fn new() -> Self {
        Self {
            number: 0,
            generators: Vec::new(),
            initial_counts: Vec::new(),
        }
    }

    /// This function adds a test generator to the subtask with the provided initial count.
    /// The generator is a function that returns a string (the test input).
    /// During test creation, at least `count` tests will be generated from this generator.
    /// Additional tests may be generated dynamically to ensure solutions fail appropriately.
    pub fn add_test<F: Fn() -> String + 'static>(&mut self, count: i32, function: F) {
        let generator = Rc::new(TestGenerator::new(function));
        self.generators.push(generator);
        self.initial_counts.push(count as usize);
    }

    /// This function adds a single test from a string.
    /// This is a convenience method that wraps the string in a generator.
    pub fn add_test_str<S: Into<String>>(&mut self, input: S) {
        let input: String = input.into();
        let func = move || input.clone();
        let generator = Rc::new(TestGenerator::new(func));
        self.generators.push(generator);
        self.initial_counts.push(1);
    }

    /// Get the total initial test count for this subtask.
    pub fn initial_test_count(&self) -> usize {
        self.initial_counts.iter().sum()
    }

    /// Generate a test from a random generator.
    pub(crate) fn generate_random_test(&self) -> Option<String> {
        if self.generators.is_empty() {
            return None;
        }
        use rand::Rng;
        let mut rng = rand::rng();
        let idx = rng.random_range(0..self.generators.len());
        Some(self.generators[idx].generate())
    }
}
