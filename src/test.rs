use crate::rng::Rng;
use crate::to_output::ToOutput;

pub struct TestGenerator<T: ToOutput> {
    function: Box<dyn Fn(&mut Rng) -> T>,
}

impl<T: ToOutput> TestGenerator<T> {
    pub fn new<F: Fn(&mut Rng) -> T + 'static>(function: F) -> Self {
        Self { function: Box::new(function) }
    }

    pub fn generate(&self, seed: u64) -> T {
        let mut rng = Rng::from_seed(seed);
        (self.function)(&mut rng)
    }
}
