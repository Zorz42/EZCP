/// A struct that represents a test generator.
/// It contains a function that generates a test.
pub struct TestGenerator {
    function: Box<dyn Fn() -> String>,
}

impl TestGenerator {
    pub fn new<F: Fn() -> String + 'static>(function: F) -> Self {
        Self { function: Box::new(function) }
    }

    pub fn generate(&self) -> String {
        (self.function)()
    }
}
