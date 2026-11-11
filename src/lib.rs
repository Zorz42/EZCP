//! Test data generation for competitive programming tasks.
//!
//! A task has an official solution and subtasks with generators. EZCP runs the
//! solution on every generated input to get the outputs, and keeps generating
//! until every partial solution fails the subtasks it is declared not to pass.
//!
//! ```no_run
//! use std::path::PathBuf;
//! use ezcp::{Counted, Subtask, Task};
//!
//! const SOLUTION: &str = r"
//! #include <iostream>
//! int main() { int n; std::cin >> n; long long sum = 0; while (n--) { int a; std::cin >> a; sum += a; } std::cout << sum << std::endl; }
//! ";
//!
//! # fn main() -> ezcp::Result<()> {
//! Task::new("Sum", &PathBuf::from("sum"))
//!     .with_solution_source(SOLUTION)
//!     .with_subtask(Subtask::new(100, "n <= 1000").with_test(10, |rng| {
//!         let n = rng.random_range(1..=1000);
//!         // Written as `n` on one line and the values on the next.
//!         Counted(rng.random_vec(n, 0..=1_000_000))
//!     }))
//!     .run()
//! # }
//! ```
//!
//! A generator returns anything that is [`ToOutput`], which there is a derive
//! for, and builds it with the help of [`Rng`], [`Graph`] and the wrappers
//! [`Counted`], [`Lines`] and [`CountedLines`].
//!
//! A generator must take all of its randomness from the [`Rng`] it is given: a
//! test is stored as its generator and seed, which have to rebuild the same
//! bytes. [Seed mode](Mode::Seeds) checks this by rebuilding every test.

// The `ToOutput` derive names `::ezcp::ToOutput`, which needs this inside the crate.
extern crate self as ezcp;

mod archiver;
mod create_tests;
mod error;
mod generate;
mod generators;
mod logger_format;
mod mode;
mod parallel;
mod partial_solution;
mod progress;
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

/// Compiles the code in the README, so that it cannot go stale.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExamples;

pub use error::{Error, Result};
pub use generators::{Graph, GraphFormat, array_generator, array_generator_custom};
pub use mode::Mode;
pub use rng::{Rng, SampleInteger, SampleUniform};
pub use subtask::Subtask;
pub use task::Task;
pub use to_output::{Counted, CountedLines, Lines, ToOutput};
