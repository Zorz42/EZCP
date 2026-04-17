use crate::test::TestGenerator;

use crate::to_output::ToOutput;
use rand::RngExt;
use std::rc::Rc;

/// Represents a problem subtask with specific constraints.
///
/// A subtask contains one or more test generators that produce input data
/// adhering to the subtask's limits.
#[derive(Default)]
pub struct Subtask<T: ToOutput> {
    pub(super) name: String,
    pub(super) points: i32,
    /// Generators that produce test inputs for this subtask
    pub(super) generators: Vec<Rc<TestGenerator<T>>>,
    /// Minimum number of tests to generate from each generator initially
    pub(super) initial_counts: Vec<usize>,
    /// Override custom `min_failures_per_solution`
    pub(super) min_failures_per_solution: Option<usize>,
}

impl<T: ToOutput> Subtask<T> {
    /// Creates a new, empty subtask.
    #[must_use]
    pub fn new(points: i32, name: &str) -> Self {
        Self {
            name: name.to_owned(),
            points,
            generators: Vec::new(),
            initial_counts: Vec::new(),
            min_failures_per_solution: None,
        }
    }

    /// Adds a random test generator to the subtask.
    ///
    /// * `count` - Initial number of tests to generate from this generator.
    /// * `function` - A closure that returns a generated input string.
    #[must_use]
    pub fn with_test<F: Fn() -> T + 'static>(mut self, count: i32, function: F) -> Self {
        let generator = Rc::new(TestGenerator::new(function));
        self.generators.push(generator);
        self.initial_counts.push(count as usize);
        self
    }

    /// Override custom `min_failures_per_solution`
    #[must_use]
    pub const fn with_min_failures(mut self, min_failures: usize) -> Self {
        self.min_failures_per_solution = Some(min_failures);
        self
    }

    /// Randomly selects one of the registered generators and produces a test input.
    ///
    /// Returns `None` if no generators are registered.
    pub(crate) fn generate_random_test(&self) -> Option<(T, usize)> {
        if self.generators.is_empty() {
            return None;
        }

        let mut rng = rand::rng();
        let idx = rng.random_range(0..self.generators.len());
        Some((self.generators[idx].generate(), idx))
    }
}
