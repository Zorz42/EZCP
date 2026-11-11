use crate::rng::Rng;
use crate::to_output::ToOutput;
use crate::whitespace::trim_whitespace_owned;

type Generator<T> = Box<dyn Fn(&mut Rng) -> T + Send + Sync>;
type Validator<T> = Box<dyn Fn(&T) + Send + Sync>;

pub struct Subtask<T: ToOutput> {
    pub(crate) name: String,
    pub(crate) points: i32,
    generators: Vec<Generator<T>>,
    /// How many tests each generator contributes up front.
    pub(crate) initial_counts: Vec<usize>,
    /// Overrides the task's.
    pub(crate) min_failures_per_solution: Option<usize>,
    pub(crate) stress_tests: usize,
    validator: Option<Validator<T>>,
}

impl<T: ToOutput> Subtask<T> {
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

    /// A generator that contributes `count` tests up front; with zero it only hunts for tests that
    /// break partial solutions. All of its randomness has to come from the `Rng`.
    #[must_use]
    pub fn with_test<F: Fn(&mut Rng) -> T + Send + Sync + 'static>(mut self, count: usize, generator: F) -> Self {
        self.generators.push(Box::new(generator));
        self.initial_counts.push(count);
        self
    }

    #[must_use]
    pub fn with_fixed_test(self, test: T) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        self.with_test(1, move |_rng| test.clone())
    }

    /// Overrides [`Task::with_min_failures`](crate::Task::with_min_failures).
    #[must_use]
    pub const fn with_min_failures(mut self, min_failures: usize) -> Self {
        self.min_failures_per_solution = Some(min_failures);
        self
    }

    /// First runs each generator `count` times through the solutions meant to pass and discards the
    /// tests, to catch generators that hang or break them.
    #[must_use]
    pub const fn with_stress_tests(mut self, count: usize) -> Self {
        self.stress_tests = count;
        self
    }

    #[must_use]
    pub fn num_generators(&self) -> usize {
        self.generators.len()
    }

    /// Is given every generated test and panics on one outside the constraints.
    #[must_use]
    pub fn with_validator<F: Fn(&T) + Send + Sync + 'static>(mut self, validator: F) -> Self {
        self.validator = Some(Box::new(validator));
        self
    }

    pub(crate) fn generate_test(&self, generator: usize, seed: u64) -> T {
        let test = self.generators[generator](&mut Rng::from_seed(seed));
        if let Some(validator) = &self.validator {
            validator(&test);
        }
        test
    }

    /// Shared by generation and serving, so a served test matches its file byte for byte.
    pub(crate) fn generate_input(&self, generator: usize, seed: u64, trim_whitespace: bool) -> String {
        let raw = self.generate_test(generator, seed).to_output();
        if trim_whitespace { trim_whitespace_owned(raw) } else { raw }
    }

    pub(crate) fn pick_generator(&self, rng: &mut Rng) -> Option<usize> {
        (!self.generators.is_empty()).then(|| rng.random_range(0..self.generators.len()))
    }
}
