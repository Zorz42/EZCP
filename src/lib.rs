#![doc = include_str!("../README.md")]

// The `ToOutput` derive names `::ezcp::ToOutput`, which needs this inside the crate.
extern crate self as ezcp;

mod archiver;
mod create_tests;
mod error;
mod generate;
mod generators;
mod logger_format;
mod mode;
mod outcome;
mod parallel;
mod partial_solution;
mod progress;
pub mod report;
mod rng;
mod runner;
mod serve;
mod stub;
mod subtask;
mod task;
#[cfg(test)]
mod tests;
mod to_output;
mod whitespace;

#[cfg(doctest)]
#[doc = include_str!("../GUIDE.md")]
struct Guide;

pub use error::{Error, Result};
pub use generators::{Graph, GraphFormat, array_generator, array_generator_custom};
pub use mode::Mode;
pub use report::Report;
pub use rng::{Rng, SampleInteger, SampleUniform};
pub use runner::cpp_runner::Compiler;
pub use subtask::Subtask;
pub use task::Task;
pub use to_output::{Counted, CountedLines, Lines, ToOutput};
