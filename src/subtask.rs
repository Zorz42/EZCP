use crate::test::TestGenerator;

use crate::to_output::ToOutput;
use rand::RngExt;
use std::rc::Rc;

/// Represents a problem subtask with specific constraints.
///
/// A subtask contains one or more test generators that produce input data
/// adhering to the subtask's limits.
pub struct Subtask<T: ToOutput> {
    pub(crate) name: String,
    pub(crate) points: i32,
    /// Generators that produce test inputs for this subtask
    generators: Vec<Rc<TestGenerator<T>>>,
    /// Minimum number of tests to generate from each generator initially
    pub(crate) initial_counts: Vec<usize>,
    /// Override custom `min_failures_per_solution`
    pub(crate) min_failures_per_solution: Option<usize>,
    /// Stress tests are just dry runs of generators and solutions.
    /// It may be ran many times (even 1000) to really make sure all solutions are correct.
    /// By default it is disabled, because it can take a lot of time.
    pub(crate) stress_tests: i32,
    /// Checker is a function that is executed when a test is generated.
    /// It should panic when the test is not within constraints.
    /// By default it does nothing.
    checker: fn(T),
}

impl<T: ToOutput> Default for Subtask<T> {
    fn default() -> Self {
        Self::new(0, "")
    }
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
            stress_tests: 0,
            checker: |_| {},
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

    #[must_use]
    pub const fn do_stress_test(mut self, num_tests: i32) -> Self {
        self.stress_tests = num_tests;
        self
    }

    #[must_use]
    pub const fn get_num_generators(&self) -> usize {
        self.generators.len()
    }

    #[must_use]
    pub fn with_checker(mut self, checker: fn(T)) -> Self {
        self.checker = checker;
        self
    }

    pub(crate) fn generate_test(&self, gen_idx: usize) -> T {
        self.generators[gen_idx].generate()
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
        Some((self.generate_test(idx), idx))
    }
}
