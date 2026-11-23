use crate::Result;
use std::path::Path;

/// Compiles C++ as a task does, into a build folder: a task with that build folder then finds the
/// programs compiled already. It may be used from several threads at once.
pub struct Compiler {
    runner: cpprunner::Runner,
}

impl Compiler {
    pub fn new(build_folder: &Path) -> Result<Self> {
        Ok(Self {
            runner: cpprunner::Runner::new(build_folder)?,
        })
    }

    /// A `CompilerError` holds the compiler's messages.
    pub fn compile(&self, source_code: &str) -> Result<()> {
        Ok(self.runner.compile(source_code)?)
    }
}
