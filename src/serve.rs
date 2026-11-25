//! Serve mode. Answers are unframed, so a stub that cannot be answered writes nothing and ends the
//! session with an error instead.

use crate::stub::{Part, Stub, stable_hash};
use crate::verdict::official_output;
use crate::{Error, Result, Task, ToOutput};
use cpprunner::Runner;
use log::{debug, warn};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

impl<T: ToOutput> Task<T> {
    pub(crate) fn serve(&self) -> Result<()> {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        self.serve_io(&mut stdin.lock(), &mut stdout.lock())
    }

    pub(crate) fn serve_io<R: Read, W: Write>(&self, input: &mut R, output: &mut W) -> Result<()> {
        // Compiled on the first request for an output: serving inputs needs no compiler.
        let mut official_solution = None;

        for line in BufReader::new(input).lines() {
            let line = line.map_err(Error::io(Path::new("stdin")))?;
            if line.trim().is_empty() {
                continue;
            }

            debug!("Request: {line}");
            let payload = self.rebuild(&Stub::parse(&line)?, &mut official_solution)?;

            // The caller may wait for this answer before sending the next stub.
            output.write_all(payload.as_bytes()).and_then(|()| output.flush()).map_err(Error::io(Path::new("stdout")))?;
        }

        Ok(())
    }

    fn rebuild(&self, stub: &Stub, official_solution: &mut Option<Runner>) -> Result<String> {
        let Some(subtask) = self.subtasks.get(stub.subtask) else {
            return Err(Error::InvalidStub {
                details: format!("there is no subtask {}; this task has {}", stub.subtask, self.subtasks.len()),
            });
        };
        if stub.generator >= subtask.num_generators() {
            return Err(Error::InvalidStub {
                details: format!("there is no generator {} in subtask {}; it has {}", stub.generator, stub.subtask, subtask.num_generators()),
            });
        }

        // A panicking generator must not take the whole server down.
        let input = self
            .generate_input(stub.subtask, stub.generator, stub.seed)
            .map_err(|err| Error::InvalidStub { details: err.to_string() })?;

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

    fn run_official_solution(&self, stub: &Stub, input: &str, official_solution: &mut Option<Runner>) -> Result<String> {
        let runner = if let Some(runner) = official_solution { runner } else { official_solution.insert(self.runner()?) };
        let result = runner.spawn(&self.solution_source, input, self.limits)?.wait()?;
        Ok(self.normalise_output(official_output(result, "on-demand generation", stub.generator + 1)?))
    }
}
