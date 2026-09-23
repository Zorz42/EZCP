//! [`Mode::Serve`](crate::Mode::Serve): answers each stub on stdin with the raw
//! test data it stands for.
//!
//! The answers are unframed, so a failure cannot be reported in the stream. A
//! stub that cannot be answered writes nothing and ends the session with an
//! error instead.

use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use crate::stub::{Part, Stub, stable_hash};
use crate::{Error, Result, Task, ToOutput};
use log::{debug, warn};
use std::io::{BufRead, BufReader, Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn stdout_error(err: std::io::Error) -> Error {
    Error::IOError { err, file: "stdout".to_owned() }
}

impl<T: ToOutput> Task<T> {
    pub(crate) fn serve(&self) -> Result<()> {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        self.serve_io(&mut stdin.lock(), &mut stdout.lock())
    }

    pub(crate) fn serve_io<R: Read, W: Write>(&self, input: &mut R, output: &mut W) -> Result<()> {
        // Compiled on the first request for an output, so serving only inputs
        // needs no compiler.
        let mut official_solution = None;

        for line in BufReader::new(input).lines() {
            let line = line.map_err(|err| Error::IOError { err, file: "stdin".to_owned() })?;
            if line.trim().is_empty() {
                continue;
            }

            debug!("Request: {line}");
            let payload = self.rebuild(&Stub::parse(&line)?, &mut official_solution)?;

            output.write_all(payload.as_bytes()).map_err(stdout_error)?;
            // The caller may wait for this answer before sending the next stub.
            output.flush().map_err(stdout_error)?;
        }

        Ok(())
    }

    fn rebuild(&self, stub: &Stub, official_solution: &mut Option<(CppRunner, ProgramHandle)>) -> Result<String> {
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

        // A panicking generator must not take the whole server down.
        let input = catch_unwind(AssertUnwindSafe(|| self.generate_input(stub.subtask, stub.generator, stub.seed))).map_err(|_panic| Error::InvalidStub {
            details: format!("generator {} of subtask {} panicked on seed {:#018x}", stub.generator, stub.subtask, stub.seed),
        })?;

        let payload = match stub.part {
            Part::Input => input,
            Part::Output => self.run_official_solution(stub, &input, official_solution)?,
        };

        if let Some(recorded) = stub.hash
            && stable_hash(&payload) != recorded
        {
            match stub.part {
                Part::Input => {
                    return Err(Error::StubMismatch {
                        details: format!(
                            "generator {} of subtask {} no longer produces the test written for seed {:#018x}; \
                             the task's generators changed after the tests were made, so they have to be regenerated",
                            stub.generator, stub.subtask, stub.seed
                        ),
                    });
                }
                // With a custom checker another output can be just as correct.
                Part::Output => warn!(
                    "The official solution produced a different output for generator {} of subtask {} on seed {:#018x} than it did when the tests were made.",
                    stub.generator, stub.subtask, stub.seed
                ),
            }
        }

        Ok(payload)
    }

    fn run_official_solution(&self, stub: &Stub, input: &str, official_solution: &mut Option<(CppRunner, ProgramHandle)>) -> Result<String> {
        let (cpp_runner, handle) = if let Some(compiled) = official_solution {
            compiled
        } else {
            let mut cpp_runner = CppRunner::new(&self.build_folder_path)?;
            let handle = cpp_runner.add_program(&self.solution_source)?;
            official_solution.insert((cpp_runner, handle))
        };

        let results = cpp_runner.check_programs(input, &[*handle], self.time_limit)?;

        match &results[0] {
            RunResult::Ok(_, output) => Ok(self.normalise_output(output)),
            RunResult::TimedOut => Err(Error::SolutionTimedOut {
                test_path: "on-demand generation".to_owned(),
                gen_id: stub.generator + 1,
            }),
            RunResult::Crashed => Err(Error::SolutionCrash {
                test_path: "on-demand generation".to_owned(),
                gen_id: stub.generator + 1,
            }),
        }
    }
}
