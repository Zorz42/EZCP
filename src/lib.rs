//! Test data generation for competitive programming tasks.
//!
//! A task has an official solution and subtasks with generators. EZCP runs the
//! solution on every generated input to get the outputs, and keeps generating
//! until every partial solution fails the subtasks it is declared not to pass.
//!
//! ```no_run
//! use std::path::PathBuf;
//!
//! const SOLUTION: &str = r"
//! #include <iostream>
//! int main() { int a, b; std::cin >> a >> b; std::cout << a + b << std::endl; }
//! ";
//!
//! # fn main() -> ezcp::Result<()> {
//! ezcp::Task::new("Sum", &PathBuf::from("sum"))
//!     .with_solution_source(SOLUTION)
//!     .with_subtask(ezcp::Subtask::new(100, "a, b <= 1000").with_test(10, |rng| {
//!         format!("{} {}\n", rng.random_range(0..=1000), rng.random_range(0..=1000))
//!     }))
//!     .run()
//! # }
//! ```
//!
//! A generator must take all of its randomness from the [`Rng`] it is given: a
//! test is stored as its generator and seed, which have to rebuild the same
//! bytes. [Seed mode](Mode::Seeds) checks this by rebuilding every test.
#![warn(missing_docs)]

// The `ToOutput` derive refers to `::ezcp::ToOutput`, which needs this inside the crate.
extern crate self as ezcp;

mod archiver;
mod create_tests;
mod error;
mod generators;
mod logger_format;
mod mode;
mod partial_solution;
mod progress;
mod rng;
mod runner;
mod serve;
mod solution;
mod stub;
mod subtask;
mod task;
mod test;
#[cfg(test)]
mod tests;
mod to_output;

pub use error::{Error, Result};
pub use generators::{Graph, array_generator, array_generator_custom, array_to_string};
pub use mode::{CliOptions, Mode, SeedChoice};
pub use rng::{Rng, SampleUniform};
pub use solution::Solution;
pub use stub::{Part, Stub, stable_hash};
pub use subtask::Subtask;
pub use task::{DEFAULT_REPRODUCIBILITY_CHECKS, DEFAULT_SEED, Task};
pub use to_output::ToOutput;
