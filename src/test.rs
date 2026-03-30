use crate::to_output::ToOutput;

/// A struct that represents a test generator.
/// It contains a function that generates a test.
pub struct TestGenerator<T: ToOutput> {
    function: Box<dyn Fn() -> T>,
}

impl<T: ToOutput> TestGenerator<T> {
    pub fn new<F: Fn() -> T + 'static>(function: F) -> Self {
        Self { function: Box::new(function) }
    }

    pub fn generate(&self) -> T {
        (self.function)()
    }
}
