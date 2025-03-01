use crate::error::{Error, Result};
use std::path::PathBuf;
use std::rc::Rc;

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

/// A struct that represents a test.
/// It contains a test generator.
/// If you write the test to multiple files, it will be
/// the same test even if the generator is non-deterministic.
pub struct Test {
    input_generator: Rc<TestGenerator>,
    input_file: Option<PathBuf>,
}

impl Test {
    pub const fn new(input_generator: Rc<TestGenerator>) -> Self {
        Self { input_generator, input_file: None }
    }

    /// Generates input and writes it to `file_path`.
    /// If `input_file` is already set, it will copy the file to `file_path`.
    pub fn generate_input(&mut self, file_path: PathBuf) -> Result<()> {
        if file_path.exists() {
            return Err(Error::TestAlreadyExists { path: file_path.to_str().unwrap_or("???").to_owned() });
        }

        if let Some(input_file) = &self.input_file {
            // copy input file to file_path
            std::fs::copy(input_file, file_path).map_err(|err| Error::IOError { err })?;
        } else {
            // generate input and write it to file_path
            let input = self.input_generator.generate();
            self.input_file = Some(file_path.clone());
            std::fs::write(file_path, input).map_err(|err| Error::IOError { err })?;
        }

        Ok(())
    }

    /// Resets the input file so that it will be generated again
    pub fn reset_input_file(&mut self) {
        self.input_file = None;
    }
}
