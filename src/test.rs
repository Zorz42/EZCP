use std::rc::Rc;

pub struct TestGenerator {
    function: Box<dyn Fn() -> String>,
}

impl TestGenerator {
    pub fn new<F>(function: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        Self { function: Box::new(function) }
    }

    pub fn generate(&self) -> String {
        (self.function)()
    }
}

/// It takes a generator instead of a test
#[derive(Clone)]
pub struct Test {
    input_generator: Rc<TestGenerator>,
    input: String,
}

impl Test {
    pub fn new(input_generator: Rc<TestGenerator>) -> Self {
        Self {
            input_generator,
            input: String::new(),
        }
    }

    pub fn generate_input(&mut self) {
        self.input = self.input_generator.generate();
    }

    pub fn get_input(&self) -> &str {
        &self.input
    }
}
