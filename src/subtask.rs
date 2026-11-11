use crate::rng::Rng;
use crate::to_output::ToOutput;
use crate::whitespace::trim_whitespace_owned;

type Generator<T> = Box<dyn Fn(&mut Rng) -> T + Send + Sync>;
type Validator<T> = Box<dyn Fn(&T) + Send + Sync>;

/// A subtask: its points, name and generators.
pub struct Subtask<T: ToOutput> {
    pub(crate) name: String,
    pub(crate) points: i32,
    generators: Vec<Generator<T>>,
    /// How many tests each generator contributes up front.
    pub(crate) initial_counts: Vec<usize>,
    /// Overrides the task's `min_failures_per_solution`.
    pub(crate) min_failures_per_solution: Option<usize>,
    pub(crate) stress_tests: usize,
    validator: Option<Validator<T>>,
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
            validator: None,
        }
    }

    /// Adds a generator that contributes `count` tests up front; with a count of
    /// zero it is only used to hunt for tests that break partial solutions.
    ///
    /// It must take all of its randomness from the [`Rng`] it is given, or its
    /// tests cannot be rebuilt from their seeds. Generators run on several
    /// threads, hence `Send + Sync`.
    #[must_use]
    pub fn with_test<F: Fn(&mut Rng) -> T + Send + Sync + 'static>(mut self, count: usize, function: F) -> Self {
        self.generators.push(Box::new(function));
        self.initial_counts.push(count);
        self
    }

    /// Adds one hand-written test, such as a sample from the statement.
    ///
    /// ```
    /// # use ezcp::Subtask;
    /// let subtask = Subtask::new(10, "samples").with_fixed_test("3\n1 2 3\n".to_owned());
    /// ```
    #[must_use]
    pub fn with_fixed_test(self, test: T) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        self.with_test(1, move |_rng| test.clone())
    }

    /// Overrides [`Task::with_min_failures`](crate::Task::with_min_failures) for
    /// this subtask.
    #[must_use]
    pub const fn with_min_failures(mut self, min_failures: usize) -> Self {
        self.min_failures_per_solution = Some(min_failures);
        self
    }

    /// Before generating, runs each generator `count` times through the
    /// solutions and discards the tests, to find generators that hang or break a
    /// solution meant to pass.
    #[must_use]
    pub const fn with_stress_tests(mut self, count: usize) -> Self {
        self.stress_tests = count;
        self
    }

    /// How many generators this subtask has.
    #[must_use]
    pub fn num_generators(&self) -> usize {
        self.generators.len()
    }

    /// Sets a function that is given every generated test and should panic if
    /// the test violates the subtask's constraints.
    ///
    /// ```
    /// # use ezcp::Subtask;
    /// let subtask = Subtask::new(100, "n <= 1000")
    ///     .with_test(5, |rng| vec![rng.random_range(1..=1000); 3])
    ///     .with_validator(|values: &Vec<i32>| assert!(values.iter().all(|value| (1..=1000).contains(value))));
    /// ```
    #[must_use]
    pub fn with_validator<F: Fn(&T) + Send + Sync + 'static>(mut self, validator: F) -> Self {
        self.validator = Some(Box::new(validator));
        self
    }

    /// # Panics
    /// Panics if `gen_idx` does not exist, or if the validator rejects the test.
    pub(crate) fn generate_test(&self, gen_idx: usize, seed: u64) -> T {
        let test = self.generators[gen_idx](&mut Rng::from_seed(seed));
        if let Some(validator) = &self.validator {
            validator(&test);
        }
        test
    }

    /// The input file of a test. Shared by generation and serving, so that a
    /// served test matches the file byte for byte.
    ///
    /// # Panics
    /// As [`Subtask::generate_test`].
    pub(crate) fn generate_input(&self, gen_idx: usize, seed: u64, trim_whitespace: bool) -> String {
        let raw = self.generate_test(gen_idx, seed).to_output();
        if trim_whitespace { trim_whitespace_owned(raw) } else { raw }
    }

    pub(crate) fn pick_generator(&self, rng: &mut Rng) -> Option<usize> {
        (!self.generators.is_empty()).then(|| rng.random_range(0..self.generators.len()))
    }
}
