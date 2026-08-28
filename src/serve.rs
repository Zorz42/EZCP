//! The on-demand test server: mode three.
//!
//! Nothing is generated up front. The task's official solution is compiled, the
//! [seed manifest](crate::manifest) is read, and then each request on stdin is
//! answered by rebuilding exactly the test it names and writing back the raw
//! bytes of one half of it: either the input, or what the official solution
//! prints for that input.
//!
//! Requests are JSON, one object per line. Answers are not: a response is the
//! bytes of the file a normal run would have written and nothing else — no
//! framing, no escaping, no newline of its own — so a judge can send the answer
//! straight into a file or into a solution's stdin.
//!
//! ```text
//! > {"command":"test","subtask":0,"test":3,"part":"input"}
//! < 5
//! < 2 4 6 8 10
//! ```
//!
//! Which half to send is what `"part"` says, and it has to say: a response
//! carries one of them, so there is no way to ask for both at once. Asking for
//! the input is also the cheap request, because the solution is never run.
//!
//! A payload carries no framing, which leaves nothing to tell a failed request
//! apart from a test whose content happens to be empty. A request that cannot be
//! answered therefore writes nothing to stdout and ends the session with an
//! error on stderr: everything already written stays valid, and the exit status
//! says the rest is not coming. `info` is the one exception to raw output — it
//! asks about the manifest rather than for a test, and is answered with a single
//! JSON line.

#[cfg(test)]
use crate::create_tests::GeneratedTest;
use crate::manifest::{Manifest, ManifestTest, stable_hash};
use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use crate::{Error, Result, Task, ToOutput};
use log::{debug, info, warn};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
#[cfg(test)]
use std::sync::Arc;

/// What a request asked the server to do.
enum Request {
    /// Describe the task and the tests the manifest holds.
    Info,
    /// Serve half of a test the manifest lists.
    Test { subtask: usize, test: usize, part: Part },
    /// Serve half of a test built from a generator and a seed given in the
    /// request, which need not appear in the manifest at all.
    Seed { subtask: usize, generator: usize, seed: u64, part: Part },
    /// Stop serving.
    Quit,
}

/// Which half of a test the caller asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    /// The test's input, byte for byte the `.in` file. The solution is not run.
    Input,
    /// What the official solution prints for that input, byte for byte the
    /// `.out` file.
    Output,
}

/// Reads the field that decides what a response carries.
fn part_field(request: &Value) -> Result<Part> {
    match request.get("part").and_then(Value::as_str) {
        Some("input") => Ok(Part::Input),
        Some("output") => Ok(Part::Output),
        Some(other) => Err(Error::InvalidArguments {
            details: format!("\"part\" is \"{other}\"; it has to be \"input\" or \"output\""),
        }),
        None => Err(Error::InvalidArguments {
            details: "\"part\" is missing; a response carries either \"input\" or \"output\", so the request has to say which".to_owned(),
        }),
    }
}

/// Reads a required index field.
fn index(request: &Value, key: &str) -> Result<usize> {
    request
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| Error::InvalidArguments {
            details: format!("\"{key}\" is missing or is not a non-negative integer"),
        })
}

/// Reads a seed, which may be written as a hexadecimal string or as a number.
///
/// Strings are what the manifest uses and what a caller should send, because a
/// JSON number cannot hold all 64 bits in every reader; a number is still
/// accepted for convenience when the seed is small.
fn seed_field(request: &Value) -> Result<u64> {
    match request.get("seed") {
        Some(Value::String(text)) => u64::from_str_radix(text.trim_start_matches("0x"), 16).map_err(|_ignored| Error::InvalidArguments {
            details: format!("\"seed\" is not a 64-bit hexadecimal number: \"{text}\""),
        }),
        Some(Value::Number(number)) => number.as_u64().ok_or_else(|| Error::InvalidArguments {
            details: "\"seed\" is not a non-negative integer".to_owned(),
        }),
        _ => Err(Error::InvalidArguments {
            details: "\"seed\" is missing".to_owned(),
        }),
    }
}

impl Request {
    /// Parses one line of the protocol.
    fn parse(line: &str) -> Result<Self> {
        let request: Value = serde_json::from_str(line).map_err(|err| Error::InvalidArguments {
            details: format!("not a JSON object: {err}"),
        })?;

        // A command is only optional in the one case where it cannot be
        // ambiguous, so that the common request stays short.
        let command = request
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_else(|| if request.get("seed").is_some() { "seed" } else { "test" });

        match command {
            "info" => Ok(Self::Info),
            "quit" | "exit" => Ok(Self::Quit),
            "test" => Ok(Self::Test {
                subtask: index(&request, "subtask")?,
                test: index(&request, "test")?,
                part: part_field(&request)?,
            }),
            "seed" => Ok(Self::Seed {
                subtask: index(&request, "subtask")?,
                generator: index(&request, "generator")?,
                seed: seed_field(&request)?,
                part: part_field(&request)?,
            }),
            other => Err(Error::InvalidArguments {
                details: format!("unknown command \"{other}\"; expected \"test\", \"seed\", \"info\" or \"quit\""),
            }),
        }
    }
}

/// Wraps a write error, stdout being the only thing this module writes to.
fn stdout_error(err: std::io::Error) -> Error {
    Error::IOError { err, file: "stdout".to_owned() }
}

impl<T: ToOutput> Task<T> {
    /// Serves tests on stdin and stdout until the input ends or a `quit` arrives.
    pub(crate) fn serve(&self) -> Result<()> {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        self.serve_io(&mut stdin.lock(), &mut stdout.lock())
    }

    /// The body of [`Task::serve`], against any reader and writer so that it can
    /// be tested without a process boundary.
    pub(crate) fn serve_io<R: Read, W: Write>(&self, input: &mut R, output: &mut W) -> Result<()> {
        let manifest = Manifest::read(&self.manifest_path)?;
        self.check_manifest_matches(&manifest)?;

        // Everything that answers a request is set up once, before the first one
        // arrives: compiling the solution takes far longer than serving a test.
        let mut cpp_runner = CppRunner::new(&self.build_folder_path)?;
        let solution_handle = cpp_runner.add_program(&self.solution_source)?;

        info!(
            "Serving {} tests of task \"{}\" from {}",
            manifest.num_tests(),
            manifest.task,
            crate::task::path_str(&self.manifest_path)
        );

        for line in BufReader::new(input).lines() {
            let line = line.map_err(|err| Error::IOError { err, file: "stdin".to_owned() })?;
            if line.trim().is_empty() {
                continue;
            }

            debug!("Request: {line}");
            // A request that cannot be answered stops the session: with nothing
            // framing a payload, a caller reading stdout has no way to be told
            // that what follows is not the test it asked for.
            match Request::parse(&line)? {
                Request::Quit => return Ok(()),
                Request::Info => writeln!(output, "{}", Self::info_response(&manifest)).map_err(stdout_error)?,
                Request::Test { subtask, test, part } => {
                    let payload = self.serve_manifest_test(&manifest, subtask, test, part, &mut cpp_runner, solution_handle)?;
                    output.write_all(payload.as_bytes()).map_err(stdout_error)?;
                }
                Request::Seed { subtask, generator, seed, part } => {
                    let payload = self.serve_generated_test(subtask, generator, seed, part, None, &mut cpp_runner, solution_handle)?;
                    output.write_all(payload.as_bytes()).map_err(stdout_error)?;
                }
            }

            // A judge is waiting on this answer before it sends the next request,
            // so nothing may sit in the buffer.
            output.flush().map_err(stdout_error)?;
        }

        Ok(())
    }

    /// Refuses a manifest that was written for something other than this binary.
    ///
    /// Serving tests from a mismatched manifest would hand out data that does not
    /// belong to the task, which is worse than refusing to start.
    fn check_manifest_matches(&self, manifest: &Manifest) -> Result<()> {
        if manifest.task != self.name {
            return Err(Error::ManifestMismatch {
                details: format!("it was written for task \"{}\", but this is task \"{}\"", manifest.task, self.name),
            });
        }

        if manifest.trim_whitespace != self.trim_whitespace {
            return Err(Error::ManifestMismatch {
                details: format!(
                    "it was written with trim_whitespace = {}, but this task has it set to {}, so served tests would not match the recorded ones",
                    manifest.trim_whitespace, self.trim_whitespace
                ),
            });
        }

        if manifest.subtasks.len() != self.subtasks.len() {
            return Err(Error::ManifestMismatch {
                details: format!("it has {} subtasks, but this task has {}", manifest.subtasks.len(), self.subtasks.len()),
            });
        }

        Ok(())
    }

    /// Describes the task and its tests.
    ///
    /// The one request whose answer is metadata rather than a test, and so the
    /// one that is still JSON.
    fn info_response(manifest: &Manifest) -> Value {
        json!({
            "ok": true,
            "task": manifest.task,
            "seed": format!("{:016x}", manifest.seed),
            "trim_whitespace": manifest.trim_whitespace,
            "time_limit": manifest.time_limit,
            "num_tests": manifest.num_tests(),
            "subtasks": manifest.subtasks.iter().map(|subtask| json!({
                "index": subtask.index,
                "points": subtask.points,
                "name": subtask.name,
                "num_tests": subtask.tests.len(),
            })).collect::<Vec<_>>(),
        })
    }

    /// Answers a request for a test the manifest lists.
    fn serve_manifest_test(&self, manifest: &Manifest, subtask: usize, test: usize, part: Part, cpp_runner: &mut CppRunner, solution_handle: ProgramHandle) -> Result<String> {
        let entry = manifest.find_test(subtask, test).ok_or_else(|| Error::InvalidArguments {
            details: format!("there is no test {test} in subtask {subtask}; the manifest has {} subtasks", manifest.subtasks.len()),
        })?;

        self.serve_generated_test(subtask, entry.generator, entry.seed, part, Some(entry), cpp_runner, solution_handle)
    }

    /// Rebuilds one test and returns the half that was asked for.
    ///
    /// `recorded` is the manifest entry when there is one; its input hash is what
    /// proves the generator still produces the test the manifest promised.
    fn serve_generated_test(
        &self,
        subtask: usize,
        generator: usize,
        seed: u64,
        part: Part,
        recorded: Option<&ManifestTest>,
        cpp_runner: &mut CppRunner,
        solution_handle: ProgramHandle,
    ) -> Result<String> {
        if subtask >= self.subtasks.len() {
            return Err(Error::InvalidArguments {
                details: format!("there is no subtask {subtask}; this task has {}", self.subtasks.len()),
            });
        }
        if generator >= self.subtasks[subtask].get_num_generators() {
            return Err(Error::InvalidArguments {
                details: format!("there is no generator {generator} in subtask {subtask}; it has {}", self.subtasks[subtask].get_num_generators()),
            });
        }

        // A generator is arbitrary user code that may well assert its way out on
        // input it does not like, and a long-running server must not die of one
        // bad request.
        let input = catch_unwind(AssertUnwindSafe(|| self.generate_input(subtask, generator, seed))).map_err(|_panic| Error::InvalidArguments {
            details: format!("generator {generator} of subtask {subtask} panicked on seed {seed:#018x}"),
        })?;

        // A test that no longer matches what was recorded means the task's
        // generators have changed since the manifest was written, and everything
        // built from that manifest is now wrong. Refusing is the only safe answer.
        if let Some(recorded) = recorded
            && stable_hash(&input) != recorded.input_hash
        {
            return Err(Error::ManifestMismatch {
                details: format!(
                    "generator {generator} of subtask {subtask} no longer produces the test recorded for seed {seed:#018x}; \
                     the task's generators changed after the manifest was written, so it has to be regenerated"
                ),
            });
        }

        if part == Part::Input {
            return Ok(input);
        }

        let output = self.run_official_solution(&input, cpp_runner, solution_handle)?;

        // The official output is only one of possibly many correct answers, so a
        // different one is not necessarily wrong - a task with a custom checker
        // may legitimately produce another. It is still worth saying, because the
        // usual cause is a solution that changed.
        if let Some(recorded) = recorded
            && stable_hash(&output) != recorded.output_hash
        {
            warn!("The official solution produced a different output for generator {generator} of subtask {subtask} on seed {seed:#018x} than it did when the manifest was written.");
        }

        Ok(output)
    }

    /// Runs the official solution on an input and returns its output, normalised
    /// exactly as test generation would have normalised it.
    fn run_official_solution(&self, input: &str, cpp_runner: &mut CppRunner, solution_handle: ProgramHandle) -> Result<String> {
        let results = cpp_runner.check_programs(input, &[solution_handle], self.time_limit)?;

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

    /// Rebuilds a test in full, as [`GeneratedTest`], without any of the
    /// protocol around it.
    ///
    /// Used by the tests, which compare a regenerated test against the file a
    /// normal run wrote for it.
    #[cfg(test)]
    pub(crate) fn regenerate_test(&self, subtask: usize, generator: usize, seed: u64, cpp_runner: &mut CppRunner, solution_handle: ProgramHandle) -> Result<GeneratedTest> {
        let input = self.generate_input(subtask, generator, seed);
        let output = self.run_official_solution(&input, cpp_runner, solution_handle)?;
        Ok(GeneratedTest {
            generator,
            seed,
            input: Arc::from(input),
            output: Arc::from(output),
        })
    }
}
