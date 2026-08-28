//! The on-demand test server: mode three.
//!
//! Nothing is generated up front. Each line of stdin is a [stub](crate::stub) —
//! the contents of a test file written by [seed mode](crate::Mode::Seeds) — and
//! the answer is the raw bytes that stub stands for, written to stdout and
//! nothing else: no framing, no escaping, no newline of its own. A stub file
//! piped in comes back out as the test file it replaces.
//!
//! ```text
//! $ ./task --serve < tests/test.01.001.in > test.in
//! $ ./task --serve < tests/test.01.001.out > test.out
//! ```
//!
//! Several stubs can be fed in at once, one per line, and the answers arrive in
//! order. The official solution is only compiled when a stub asks for an output,
//! so rebuilding an input costs nothing but the generator.
//!
//! Because a payload carries no framing, there is nothing to tell a failed
//! request apart from a test whose content happens to be empty. A stub that
//! cannot be answered therefore writes nothing to stdout and ends the session
//! with an error on stderr: everything already written stays valid, and the exit
//! status says the rest is not coming.

use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use crate::stub::{Part, Stub, stable_hash};
use crate::{Error, Result, Task, ToOutput};
use log::{debug, warn};
use std::io::{BufRead, BufReader, Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Wraps a write error, stdout being the only thing this module writes to.
fn stdout_error(err: std::io::Error) -> Error {
    Error::IOError { err, file: "stdout".to_owned() }
}

impl<T: ToOutput> Task<T> {
    /// Serves tests on stdin and stdout until the input ends.
    pub(crate) fn serve(&self) -> Result<()> {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        self.serve_io(&mut stdin.lock(), &mut stdout.lock())
    }

    /// The body of [`Task::serve`], against any reader and writer so that it can
    /// be tested without a process boundary.
    pub(crate) fn serve_io<R: Read, W: Write>(&self, input: &mut R, output: &mut W) -> Result<()> {
        let mut cpp_runner = CppRunner::new(&self.build_folder_path)?;
        // Compiling the solution takes far longer than rebuilding a test, and a
        // session that only ever asks for inputs never needs it at all.
        let mut solution_handle = None;

        for line in BufReader::new(input).lines() {
            let line = line.map_err(|err| Error::IOError { err, file: "stdin".to_owned() })?;
            if line.trim().is_empty() {
                continue;
            }

            debug!("Request: {line}");
            // A stub that cannot be answered stops the session: with nothing
            // framing a payload, a caller reading stdout has no way to be told
            // that what follows is not the test it asked for.
            let payload = self.rebuild(&Stub::parse(&line)?, &mut cpp_runner, &mut solution_handle)?;

            output.write_all(payload.as_bytes()).map_err(stdout_error)?;
            // A judge is waiting on this answer before it sends the next stub, so
            // nothing may sit in the buffer.
            output.flush().map_err(stdout_error)?;
        }

        Ok(())
    }

    /// Rebuilds the half of a test that a stub stands for.
    fn rebuild(&self, stub: &Stub, cpp_runner: &mut CppRunner, solution_handle: &mut Option<ProgramHandle>) -> Result<String> {
        if stub.subtask >= self.subtasks.len() {
            return Err(Error::InvalidStub {
                details: format!("there is no subtask {}; this task has {}", stub.subtask, self.subtasks.len()),
            });
        }
        if stub.generator >= self.subtasks[stub.subtask].get_num_generators() {
            return Err(Error::InvalidStub {
                details: format!(
                    "there is no generator {} in subtask {}; it has {}",
                    stub.generator,
                    stub.subtask,
                    self.subtasks[stub.subtask].get_num_generators()
                ),
            });
        }

        // A generator is arbitrary user code that may well assert its way out on
        // input it does not like, and a long-running server must not die of one
        // bad stub.
        let input = catch_unwind(AssertUnwindSafe(|| self.generate_input(stub.subtask, stub.generator, stub.seed))).map_err(|_panic| Error::InvalidStub {
            details: format!("generator {} of subtask {} panicked on seed {:#018x}", stub.generator, stub.subtask, stub.seed),
        })?;

        let payload = match stub.part {
            Part::Input => input,
            Part::Output => self.run_official_solution(&input, cpp_runner, solution_handle)?,
        };

        if let Some(recorded) = stub.hash
            && stable_hash(&payload) != recorded
        {
            match stub.part {
                // The generators have changed since the stub was written, so
                // everything built from these stubs is now wrong. Handing out a
                // test that is not the one that was verified is worse than
                // refusing.
                Part::Input => {
                    return Err(Error::StubMismatch {
                        details: format!(
                            "generator {} of subtask {} no longer produces the test written for seed {:#018x}; \
                             the task's generators changed after the tests were made, so they have to be regenerated",
                            stub.generator, stub.subtask, stub.seed
                        ),
                    });
                }
                // The official output is only one of possibly many correct
                // answers, so a different one is not necessarily wrong - a task
                // with a custom checker may legitimately produce another. It is
                // still worth saying, because the usual cause is a solution that
                // changed.
                Part::Output => warn!(
                    "The official solution produced a different output for generator {} of subtask {} on seed {:#018x} than it did when the tests were made.",
                    stub.generator, stub.subtask, stub.seed
                ),
            }
        }

        Ok(payload)
    }

    /// Runs the official solution on an input and returns its output, normalised
    /// exactly as test generation would have normalised it.
    ///
    /// The solution is compiled the first time it is needed and kept afterwards.
    fn run_official_solution(&self, input: &str, cpp_runner: &mut CppRunner, solution_handle: &mut Option<ProgramHandle>) -> Result<String> {
        let handle = match *solution_handle {
            Some(handle) => handle,
            None => *solution_handle.insert(cpp_runner.add_program(&self.solution_source)?),
        };

        let results = cpp_runner.check_programs(input, &[handle], self.time_limit)?;

        match &results[0] {
            RunResult::Ok(_, output) => Ok(self.normalise_output(output)),
            RunResult::TimedOut => Err(Error::SolutionTimedOut {
                test_path: "on-demand generation".to_owned(),
                gen_id: 0,
            }),
            RunResult::Crashed => Err(Error::SolutionCrash {
                test_path: "on-demand generation".to_owned(),
                gen_id: 0,
            }),
        }
    }
}
