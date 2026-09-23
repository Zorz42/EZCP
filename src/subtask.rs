use crate::rng::Rng;
use crate::test::TestGenerator;

use crate::to_output::ToOutput;

/// A subtask: its points, name and generators.
pub struct Subtask<T: ToOutput> {
    pub(crate) name: String,
    pub(crate) points: i32,
    generators: Vec<TestGenerator<T>>,
    /// How many tests each generator contributes up front.
    pub(crate) initial_counts: Vec<usize>,
    /// Overrides the task's `min_failures_per_solution`.
    pub(crate) min_failures_per_solution: Option<usize>,
    pub(crate) stress_tests: i32,
    checker: fn(&T),
}

impl<T: ToOutput> Default for Subtask<T> {
    fn default() -> Self {
        Self::new(0, "")
    }
}

impl<T: ToOutput> Subtask<T> {
    /// Creates a subtask without generators.
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

    /// Adds a generator that contributes `count` tests up front. It must take
    /// all of its randomness from the [`Rng`] it is given, or its tests cannot
    /// be rebuilt from their seeds.
    ///
    /// # Panics
    /// Panics if `count` is negative.
    #[must_use]
    pub fn with_test<F: Fn(&mut Rng) -> T + 'static>(mut self, count: i32, function: F) -> Self {
        assert!(count >= 0, "a generator cannot produce {count} tests");
        self.generators.push(TestGenerator::new(function));
        self.initial_counts.push(count as usize);
        self
    }

    /// Overrides [`Task::with_min_failures`](crate::Task::with_min_failures) for
    /// this subtask.
    #[must_use]
    pub const fn with_min_failures(mut self, min_failures: usize) -> Self {
        self.min_failures_per_solution = Some(min_failures);
        self
    }

    /// Before generating, runs each generator `num_tests` times through the
    /// solutions and discards the tests, to find generators that hang or break a
    /// solution meant to pass.
    #[must_use]
    pub const fn do_stress_test(mut self, num_tests: i32) -> Self {
        self.stress_tests = num_tests;
        self
    }

    /// How many generators this subtask has.
    #[must_use]
    pub const fn get_num_generators(&self) -> usize {
        self.generators.len()
    }

    /// Sets a function that is given every generated test and should panic if
    /// the test violates the subtask's constraints.
    #[must_use]
    pub fn with_checker(mut self, checker: fn(&T)) -> Self {
        self.checker = checker;
        self
    }

    /// # Panics
    /// Panics if `gen_idx` does not exist.
    pub(crate) fn generate_test(&self, gen_idx: usize, seed: u64) -> T {
        let res = self.generators[gen_idx].generate(seed);
        (self.checker)(&res);
        res
    }

    pub(crate) fn pick_generator(&self, rng: &mut Rng) -> Option<usize> {
        if self.generators.is_empty() {
            return None;
        }
        Some(rng.random_range(0..self.generators.len()))
    }
}
