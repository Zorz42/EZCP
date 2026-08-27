//! The on-demand test server: mode three.
//!
//! Nothing is generated up front. The task's official solution is compiled, the
//! [seed manifest](crate::manifest) is read, and then each request on stdin is
//! answered by rebuilding exactly the test it names and, if asked for, running
//! the solution on it to get the expected output.
//!
//! The protocol is JSON Lines: one JSON object per line in, one per line out.
//! JSON is what makes the whitespace of a test survive the trip — every space,
//! tab and newline is carried inside a JSON string and comes back byte for byte,
//! with no line-based framing to mangle it. A served test is identical to the
//! file a normal run would have written.
//!
//! ```text
//! > {"command":"test","subtask":0,"test":3}
//! < {"ok":true,"subtask":0,"test":3,"generator":1,"seed":"...","input":"5\n2 4 6 8 10\n","output":"20\n"}
//! ```
//!
//! Requests are answered in the order they arrive, and a request that cannot be
//! answered produces an error object rather than ending the session: a judge
//! asking for a test that does not exist should not have to restart the server.

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
    /// Serve a test the manifest lists.
    Test { subtask: usize, test: usize, parts: Parts },
    /// Serve a test built from a generator and a seed given in the request, which
    /// need not appear in the manifest at all.
    Seed { subtask: usize, generator: usize, seed: u64, parts: Parts },
    /// Stop serving.
    Quit,
}

/// Which halves of a test the caller wants back.
///
/// Asking for the input alone skips running the solution, which is the expensive
/// half of answering a request.
#[derive(Clone, Copy)]
struct Parts {
    input: bool,
    output: bool,
}

impl Default for Parts {
    fn default() -> Self {
        Self { input: true, output: true }
    }
}

/// Reads an optional boolean field, defaulting to what [`Parts`] does.
fn wanted(request: &Value, key: &str, default: bool) -> bool {
    request.get(key).and_then(Value::as_bool).unwrap_or(default)
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

        let parts = Parts {
            input: wanted(&request, "input", true),
            output: wanted(&request, "output", true),
        };

        match command {
            "info" => Ok(Self::Info),
            "quit" | "exit" => Ok(Self::Quit),
            "test" => Ok(Self::Test {
                subtask: index(&request, "subtask")?,
                test: index(&request, "test")?,
                parts,
            }),
            "seed" => Ok(Self::Seed {
                subtask: index(&request, "subtask")?,
                generator: index(&request, "generator")?,
                seed: seed_field(&request)?,
                parts,
            }),
            other => Err(Error::InvalidArguments {
                details: format!("unknown command \"{other}\"; expected \"test\", \"seed\", \"info\" or \"quit\""),
            }),
        }
    }
}

/// Renders an error as the protocol's failure object.
fn error_response(message: &str) -> Value {
    json!({ "ok": false, "error": message })
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
            let response = match Request::parse(&line) {
                Ok(Request::Quit) => return Ok(()),
                Ok(Request::Info) => Self::info_response(&manifest),
                Ok(Request::Test { subtask, test, parts }) => {
                    // Errors here are about this one request, so they are reported
                    // in the response and the server keeps going.
                    self.serve_manifest_test(&manifest, subtask, test, parts, &mut cpp_runner, solution_handle)
                        .unwrap_or_else(|err| error_response(&err.to_string()))
                }
                Ok(Request::Seed { subtask, generator, seed, parts }) => self
                    .serve_generated_test(subtask, generator, seed, parts, None, &mut cpp_runner, solution_handle)
                    .map_or_else(|err| error_response(&err.to_string()), |(response, _built)| response),
                Err(err) => error_response(&err.to_string()),
            };

            writeln!(output, "{response}").map_err(|err| Error::IOError { err, file: "stdout".to_owned() })?;
            // A judge is waiting on this answer before it sends the next request,
            // so nothing may sit in the buffer.
            output.flush().map_err(|err| Error::IOError { err, file: "stdout".to_owned() })?;
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
    fn serve_manifest_test(&self, manifest: &Manifest, subtask: usize, test: usize, parts: Parts, cpp_runner: &mut CppRunner, solution_handle: ProgramHandle) -> Result<Value> {
        let entry = manifest.find_test(subtask, test).ok_or_else(|| Error::InvalidArguments {
            details: format!("there is no test {test} in subtask {subtask}; the manifest has {} subtasks", manifest.subtasks.len()),
        })?;

        let (mut response, built) = self.serve_generated_test(subtask, entry.generator, entry.seed, parts, Some(entry), cpp_runner, solution_handle)?;

        if let Some(object) = response.as_object_mut() {
            object.insert("test".to_owned(), json!(test));
            object.insert("input_file".to_owned(), json!(entry.input_file));
            object.insert("output_file".to_owned(), json!(entry.output_file));

            // The official output is only one of possibly many correct answers, so
            // a different one is not necessarily wrong - a task with a custom
            // checker may legitimately produce another. It is still worth saying,
            // because the usual cause is a solution that changed.
            if let Some(output) = &built.output
                && stable_hash(output) != entry.output_hash
            {
                warn!("The official solution produced a different output than it did when subtask {subtask} test {test} was recorded.");
                object.insert("output_changed".to_owned(), json!(true));
            }
        }

        Ok(response)
    }

    /// Rebuilds one test and packages it as a response.
    ///
    /// `recorded` is the manifest entry when there is one; its input hash is what
    /// proves the generator still produces the test the manifest promised.
    fn serve_generated_test(
        &self,
        subtask: usize,
        generator: usize,
        seed: u64,
        parts: Parts,
        recorded: Option<&ManifestTest>,
        cpp_runner: &mut CppRunner,
        solution_handle: ProgramHandle,
    ) -> Result<(Value, BuiltTest)> {
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

        let output = if parts.output {
            Some(self.run_official_solution(&input, cpp_runner, solution_handle)?)
        } else {
            None
        };

        let mut response = json!({
            "ok": true,
            "subtask": subtask,
            "generator": generator,
            "seed": format!("{seed:016x}"),
        });

        if let Some(object) = response.as_object_mut() {
            if parts.input {
                object.insert("input".to_owned(), json!(input));
            }
            if let Some(output) = &output {
                object.insert("output".to_owned(), json!(output));
            }
        }

        Ok((response, BuiltTest { output }))
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

/// The parts of a rebuilt test the response does not carry back on its own.
struct BuiltTest {
    output: Option<String>,
}
