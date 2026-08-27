use crate::rng::Rng;
use crate::test::TestGenerator;

use crate::to_output::ToOutput;

/// Represents a problem subtask with specific constraints.
///
/// A subtask contains one or more test generators that produce input data
/// adhering to the subtask's limits.
pub struct Subtask<T: ToOutput> {
    pub(crate) name: String,
    pub(crate) points: i32,
    /// Generators that produce test inputs for this subtask
    generators: Vec<TestGenerator<T>>,
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
    checker: fn(&T),
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
    /// * `function` - A closure that turns a seeded [`Rng`] into a generated input.
    ///
    /// The closure has to take **all** of its randomness from the [`Rng`] it is
    /// given. A generator that reaches for another source produces a test that
    /// cannot be rebuilt from its seed, which quietly breaks
    /// [seed mode](crate::Mode::Seeds) and the on-demand server.
    ///
    /// # Panics
    /// Panics if `count` is negative, which would otherwise wrap around into an
    /// effectively endless number of tests to generate.
    #[must_use]
    pub fn with_test<F: Fn(&mut Rng) -> T + 'static>(mut self, count: i32, function: F) -> Self {
        assert!(count >= 0, "a generator cannot produce {count} tests");
        self.generators.push(TestGenerator::new(function));
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
    pub fn with_checker(mut self, checker: fn(&T)) -> Self {
        self.checker = checker;
        self
    }

    /// Generates the test that generator `gen_idx` produces from `seed`.
    ///
    /// # Panics
    /// Panics if `gen_idx` does not name a generator of this subtask.
    pub(crate) fn generate_test(&self, gen_idx: usize, seed: u64) -> T {
        let res = self.generators[gen_idx].generate(seed);
        (self.checker)(&res);
        res
    }

    /// Picks one of the registered generators at random.
    ///
    /// Returns `None` if no generators are registered.
    pub(crate) fn pick_generator(&self, rng: &mut Rng) -> Option<usize> {
        if self.generators.is_empty() {
            return None;
        }
        Some(rng.random_range(0..self.generators.len()))
    }
}
