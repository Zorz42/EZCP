use crate::test::TestGenerator;

use rand::Rng;
use std::rc::Rc;

/// Represents a problem subtask with specific constraints.
///
/// A subtask contains one or more test generators that produce input data
/// adhering to the subtask's limits.
#[derive(Default)]
pub struct Subtask {
    /// The index of the subtask (0-indexed)
    pub(super) number: usize,
    /// Generators that produce test inputs for this subtask
    pub(super) generators: Vec<Rc<TestGenerator>>,
    /// Minimum number of tests to generate from each generator initially
    pub(super) initial_counts: Vec<usize>,
    
    pub(super) name: String,
}

impl Subtask {
    /// Creates a new, empty subtask.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            ..Self::default()
        }
    }

    /// Adds a random test generator to the subtask.
    ///
    /// * `count` - Initial number of tests to generate from this generator.
    /// * `function` - A closure that returns a generated input string.
    #[must_use]
    pub fn with_test<F: Fn() -> String + 'static>(mut self, count: i32, function: F) -> Self {
        let generator = Rc::new(TestGenerator::new(function));
        self.generators.push(generator);
        self.initial_counts.push(count as usize);
        self
    }

    /// Randomly selects one of the registered generators and produces a test input.
    ///
    /// Returns `None` if no generators are registered.
    pub(crate) fn generate_random_test(&self) -> Option<String> {
        if self.generators.is_empty() {
            return None;
        }

        let mut rng = rand::rng();
        let idx = rng.random_range(0..self.generators.len());
        Some(self.generators[idx].generate())
    }
}
