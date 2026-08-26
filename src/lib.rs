//! EZCP is a framework for building the test data of a competitive programming
//! task.
//!
//! You describe a task as a set of [`Subtask`]s, each with its own generators, and
//! give it a correct solution. EZCP compiles the solution, runs it on every
//! generated input to produce the expected output, writes the tests out and packs
//! them into a zip file.
//!
//! Adding partial solutions is what makes the test data worth trusting. A partial
//! solution declares which subtasks it is supposed to pass; EZCP then keeps
//! generating tests until every subtask it is *not* supposed to pass actually
//! rejects it, and reports an error if it cannot find such a test. A partial
//! solution that passes a subtask it should have failed is reported too.
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
//!     .with_subtask(ezcp::Subtask::new(100, "a, b <= 1000").with_test(10, || {
//!         format!("{} {}\n", rand::random_range(0..=1000), rand::random_range(0..=1000))
//!     }))
//!     .run()
//! # }
//! ```
//!
//! Inputs do not have to be built as strings: any type that implements
//! [`ToOutput`] can be returned from a generator, and the trait can be derived for
//! a struct that mirrors the input format. [`Graph`] and [`array_generator`] cover
//! the two shapes that come up most often.
//!
//! Running a task needs a C++ compiler on `PATH`; see the README for what to
//! install on each platform.

mod archiver;
mod create_tests;
mod error;
mod generators;
mod logger_format;
mod partial_solution;
mod progress;
mod runner;
mod solution;
mod subtask;
mod task;
mod test;
#[cfg(test)]
mod tests;
mod to_output;

pub use error::{Error, Result};
pub use generators::{Graph, array_generator, array_generator_custom, array_to_string};
pub use solution::Solution;
pub use subtask::Subtask;
pub use task::Task;
pub use to_output::ToOutput;
