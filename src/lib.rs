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
//!     .with_subtask(ezcp::Subtask::new(100, "a, b <= 1000").with_test(10, |rng| {
//!         format!("{} {}\n", rng.random_range(0..=1000), rng.random_range(0..=1000))
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
//! # Generators must take their randomness from the `Rng` they are given
//!
//! Every generator is handed a seeded [`Rng`], and everything it produces has to
//! come from that one generator. This is what makes a test reproducible: a test is
//! identified by nothing more than the generator that made it and the seed it was
//! run with, so the same pair always gives back the same bytes — on another
//! machine, in another year, in another build.
//!
//! A generator that reaches for a different source of randomness, or that captures
//! a value drawn while the task was being described, still compiles and still
//! produces tests. It just produces tests that cannot be rebuilt. Nothing in the
//! type system can prevent that, so [seed mode](Mode::Seeds) goes looking for it
//! instead: every finished test is rebuilt from its seed
//! [`DEFAULT_REPRODUCIBILITY_CHECKS`] times over and compared against what was
//! generated, and a generator that does not agree with itself fails the run rather
//! than leaving behind stubs that lie. See
//! [`Task::with_reproducibility_checks`].
//!
//! # Three ways to run a task
//!
//! [`Task::run`] takes the mode from the command line, so one compiled task binary
//! covers all three. [`Task::run_mode`] chooses in code instead.
//!
//! * **Files** (no arguments, [`Mode::Files`]) — the usual thing: generate the
//!   tests, write them out, archive them.
//! * **Seeds** (`--seeds`, [`Mode::Seeds`]) — generate and verify exactly as
//!   above, and write the same test set, except that each file holds the
//!   [stub](Stub) that rebuilds the test rather than the test itself. Every test
//!   is still produced, run against the official solution and used to hunt for
//!   counterexamples; none of the data reaches the disk. A task can then have far
//!   more tests than there is room to store. Each finished test is also rebuilt
//!   from its seed several times over, to prove the seed is worth writing down.
//! * **Serve** (`--serve`, [`Mode::Serve`]) — read stubs on stdin and answer each
//!   with the raw bytes it stands for. Piping a stub file in gives back the file
//!   a normal run would have written, byte for byte, whitespace included, with no
//!   framing around it.
//!
//! This is what lets an online judge hold a whole task's test data as a few
//! kilobytes of stubs and rebuild any individual test, deterministically, at the
//! moment it needs it.
//!
//! Running a task needs a C++ compiler on `PATH`; see the README for what to
//! install on each platform.
#![warn(missing_docs)]

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
