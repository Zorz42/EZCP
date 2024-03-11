use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::rc::Rc;

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

/// It takes a generator instead of a test
pub struct Test {
    input_generator: Rc<TestGenerator>,
    input_file: Option<PathBuf>,
}

impl Test {
    pub fn new(input_generator: Rc<TestGenerator>) -> Self {
        Self { input_generator, input_file: None }
    }

    pub fn generate_input(&mut self, file_path: &Path) -> Result<()> {
        if file_path.exists() {
            bail!("File already exists: {:?}", file_path);
        }

        if let Some(input_file) = &self.input_file {
            // copy input file to file_path
            std::fs::copy(input_file, file_path)?;
        } else {
            // generate input and write it to file_path
            let input = self.input_generator.generate();
            self.input_file = Some(file_path.to_path_buf());
            std::fs::write(file_path, input)?;
        }

        Ok(())
    }

    pub fn reset_input_file(&mut self) {
        self.input_file = None;
    }
}
