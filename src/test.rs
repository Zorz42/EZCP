use std::path::PathBuf;
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
    input_file: Option<PathBuf>,
}

impl Test {
    pub fn new(input_generator: Rc<TestGenerator>) -> Self {
        Self { input_generator, input_file: None }
    }

    pub fn generate_input(&mut self, file_path: PathBuf) {
        if let Some(input_file) = &self.input_file {
            // copy input file to file_path
            std::fs::copy(input_file, file_path.clone()).expect("Failed to copy input file");
        } else {
            // generate input and write it to file_path
            let input = self.input_generator.generate();
            std::fs::write(file_path.clone(), &input).expect("Failed to write input file");
        }
    }
}
