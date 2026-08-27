use crate::rng::Rng;
use crate::to_output::ToOutput;

/// A struct that represents a test generator.
/// It contains a function that generates a test.
///
/// The function is handed a seeded [`Rng`] and must take all of its randomness
/// from it. That is what lets a test be identified by nothing more than the
/// generator it came from and the seed it was given, which is what the seed
/// manifest and the on-demand server are built on.
pub struct TestGenerator<T: ToOutput> {
    function: Box<dyn Fn(&mut Rng) -> T>,
}

impl<T: ToOutput> TestGenerator<T> {
    pub fn new<F: Fn(&mut Rng) -> T + 'static>(function: F) -> Self {
        Self { function: Box::new(function) }
    }

    /// Generates the test belonging to `seed`.
    ///
    /// The generator gets a generator of its own, seeded with exactly this value,
    /// so the result does not depend on how many tests were generated before it.
    pub fn generate(&self, seed: u64) -> T {
        let mut rng = Rng::from_seed(seed);
        (self.function)(&mut rng)
    }
}
