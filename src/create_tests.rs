use crate::Error::SolutionFailed;
use crate::Result;
use crate::progress::ScopedProgressBar;
use crate::runner::cpp_runner::{CppRunner, ProgramHandle};
use crate::runner::exec_runner::RunResult;
use crate::task::path_str;
use crate::{Error, Subtask, Task, ToOutput};
use log::{error, info, warn};
use rand::prelude::SliceRandom;
use std::collections::HashSet;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

fn trim_whitespace(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_block = false;
    let mut has_newline = false;

    for c in input.chars() {
        if c.is_whitespace() {
            in_block = true;
            if c == '\n' {
                has_newline = true;
            }
        } else {
            // Separate from the previous run of whitespace, unless nothing has been
            // written yet: leading whitespace is dropped rather than turned into an
            // indent or a blank first line.
            if in_block && !result.is_empty() {
                result.push(if has_newline { '\n' } else { ' ' });
            }
            in_block = false;
            has_newline = false;
            result.push(c);
        }
    }

    // ensure trailing newline
    if !result.ends_with('\n') {
        result.push('\n');
    }

    result
}

/// How many times in a row a generator may repeat a test it has already produced
/// before the initial batch gives up on it.
///
/// A generator with a small range runs out of distinct tests long before it has
/// produced the requested count, and without a bound it would spin forever.
const MAX_REPEATED_TESTS: usize = 100;

fn hash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

impl<T: ToOutput> Task<T> {
    pub(super) fn create_tests_for_subtask(
        &self,
        subtask_idx: usize,
        subtask: &Subtask<T>,
        global_test_id: &mut i32,
        all_test_files: &mut Vec<Vec<(PathBuf, PathBuf)>>,
        solution_handles: &[ProgramHandle],
        solution_handle: ProgramHandle,
        cpp_runner: &mut CppRunner,
    ) -> Result<()> {
        let mut good_solution_handles = Vec::new();
        let mut bad_solution_handles = Vec::new();
        for (i, solution) in self.solutions.iter().enumerate() {
            if solution.passes_subtasks.contains(&subtask_idx) {
                good_solution_handles.push((i, solution_handles[i]));
            } else {
                bad_solution_handles.push(solution_handles[i]);
            }
        }

        let mut tried_inputs = HashSet::new();
        let mut subtask_tests = Vec::new();
        let mut robust_found_count = 0;

        let total_initial: usize = subtask.initial_counts.iter().sum();
        let target_robust = if bad_solution_handles.is_empty() {
            0
        } else {
            subtask.min_failures_per_solution.unwrap_or(self.min_failures_per_solution)
        };

        let found_count_progress_bar = ScopedProgressBar::new(&self.logger, (total_initial + target_robust) as u64);
        let tries_progress_bar = ScopedProgressBar::new(&self.logger, self.max_tries as u64);

        // Phase 1 (optional): Stress tests
        if subtask.stress_tests != 0 {
            for gen_idx in 0..subtask.get_num_generators() {
                info!("Stress testing generator {gen_idx}");
                let stress_testing_progress_bar = ScopedProgressBar::new(&self.logger, subtask.stress_tests as u64);
                for _ in 0..subtask.stress_tests {
                    let mut test_str = subtask.generate_test(gen_idx).to_output();
                    if self.trim_whitespace {
                        test_str = trim_whitespace(&test_str);
                    }

                    stress_testing_progress_bar.inc(1);

                    self.is_robust_test(&test_str, solution_handle, &good_solution_handles, &[], cpp_runner, subtask_idx, gen_idx)?;
                }
            }
        }

        // Phase 2: Initial tests from each generator (only good solutions must pass)
        for gen_idx in 0..subtask.get_num_generators() {
            let needed = subtask.initial_counts.get(gen_idx).copied().unwrap_or(0);
            let mut got = 0;
            let mut fails = 0;
            while got < needed && fails < MAX_REPEATED_TESTS {
                let mut candidate = subtask.generate_test(gen_idx).to_output();
                if self.trim_whitespace {
                    candidate = trim_whitespace(&candidate);
                }
                // Each test must be unique within the subtask
                if !tried_inputs.insert(hash_string(&candidate)) {
                    fails += 1;
                    continue;
                }

                // Only good solutions are checked here (no bad_progs passed)
                let Some(main_output) = self.is_robust_test(&candidate, solution_handle, &good_solution_handles, &[], cpp_runner, subtask_idx, gen_idx)? else {
                    unreachable!("is_robust_test with no bad progs should always return Some or Err")
                };
                subtask_tests.push((candidate, main_output));
                found_count_progress_bar.inc(1);
                got += 1;
            }
            if got < needed {
                warn!(
                    "Generator {gen_idx} of subtask {} produced only {got} of {needed} tests, because it kept repeating tests it had already generated.",
                    subtask_idx + 1
                );
            }
        }

        // Phase 3: Robust tests (failing bad solutions)
        let mut supplemental_tries = 0;
        while robust_found_count < target_robust && supplemental_tries < self.max_tries {
            supplemental_tries += 1;
            tries_progress_bar.inc(1);
            let Some((candidate, gen_idx)) = subtask.generate_random_test() else { break };
            let mut candidate = candidate.to_output();
            if self.trim_whitespace {
                candidate = trim_whitespace(&candidate);
            }
            if !tried_inputs.insert(hash_string(&candidate)) {
                continue;
            }

            if let Some(main_output) = self.is_robust_test(&candidate, solution_handle, &good_solution_handles, &bad_solution_handles, cpp_runner, subtask_idx, gen_idx)? {
                subtask_tests.push((candidate, main_output));
                robust_found_count += 1;
                supplemental_tries = 0;
                found_count_progress_bar.inc(1);
                tries_progress_bar.reset();
            }
        }

        if robust_found_count < target_robust {
            error!("Could not find enough robust tests for Subtask {} (found {}/{})", subtask_idx + 1, robust_found_count, target_robust);
        }

        // Shuffle all tests for this subtask
        let mut rng = rand::rng();
        subtask_tests.shuffle(&mut rng);

        all_test_files.push(self.write_subtask_tests(subtask_idx, subtask_tests, global_test_id)?);
        Ok(())
    }

    /// Writes the finished tests of one subtask to disk and returns the
    /// (input, output) paths they went to.
    fn write_subtask_tests(&self, subtask_idx: usize, tests: Vec<(String, String)>, global_test_id: &mut i32) -> Result<Vec<(PathBuf, PathBuf)>> {
        let mut subtask_files = Vec::new();

        for (test_id_in_subtask, (input, output)) in tests.into_iter().enumerate() {
            let input_path = self.get_input_file_path(*global_test_id, subtask_idx as i32, test_id_in_subtask as i32);
            let output_path = self.get_output_file_path(*global_test_id, subtask_idx as i32, test_id_in_subtask as i32);

            // The names come from user supplied closures, and two tests that map
            // to the same name would silently overwrite each other here and then
            // both end up in the archive under that one name. The test directory
            // starts out empty, so anything already there is from this run.
            for path in [&input_path, &output_path] {
                if path.exists() {
                    return Err(Error::TestAlreadyExists { path: path_str(path) });
                }
            }

            fs::write(&input_path, &input).map_err(|err| Error::IOError { err, file: path_str(&input_path) })?;
            fs::write(&output_path, output).map_err(|err| Error::IOError { err, file: path_str(&output_path) })?;

            subtask_files.push((input_path, output_path));
            *global_test_id += 1;
        }

        Ok(subtask_files)
    }

    /// Checks if a candidate test input effectively distinguishes between the correct solution
    /// and a set of "bad" solutions.
    ///
    /// A test is considered robust if:
    /// 1. All "good" solutions (including main) produce the same valid response.
    /// 2. Every "bad" solution either TLEs, crashes, or produces a different output.
    fn is_robust_test(
        &self,
        input: &str,
        main_prog: ProgramHandle,
        good_progs: &[(usize, ProgramHandle)],
        bad_progs: &[ProgramHandle],
        runner: &mut CppRunner,
        subtask_idx: usize,
        gen_idx: usize,
    ) -> Result<Option<String>> {
        let mut all_progs = vec![main_prog];
        for &(_, handle) in good_progs {
            all_progs.push(handle);
        }
        all_progs.extend_from_slice(bad_progs);

        // Run all solutions in parallel
        let results = runner.check_programs(input, &all_progs, self.time_limit)?;

        let write_bad_test = || -> Result<()> {
            let write_path = self.problem_path.join("failing_test.in");
            fs::write(write_path.clone(), input).map_err(move |err| Error::IOError { file: path_str(&write_path), err })?;
            Ok(())
        };

        // Correct (Main) Solution Result
        let mut correct_output = match &results[0] {
            RunResult::Ok(_, output) => output.trim().to_owned() + "\n",
            RunResult::TimedOut => {
                write_bad_test()?;
                return Err(Error::SolutionTimedOut {
                    test_path: "generation phase".to_owned(),
                    gen_id: gen_idx + 1,
                });
            }
            RunResult::Crashed => {
                write_bad_test()?;
                return Err(Error::SolutionCrash {
                    test_path: "generation phase".to_owned(),
                    gen_id: gen_idx + 1,
                });
            }
        };

        if !(self.checker)(input, &correct_output, &correct_output) {
            write_bad_test()?;
            return Err(SolutionFailed {
                test_path: "generation phase".to_owned(),
                gen_id: gen_idx + 1,
            });
        }

        if self.trim_whitespace {
            correct_output = trim_whitespace(&correct_output);
        }

        // Ensure all other "good" solutions pass and match main output
        for (i, &(sol_idx, _)) in good_progs.iter().enumerate() {
            match &results[i + 1] {
                RunResult::Ok(_, output) if (self.checker)(input, &correct_output, output) => {}
                result => {
                    let write_path = self.problem_path.join("failing_test.in");
                    let official_output_write_path = self.problem_path.join("failing_test_correct_output.out");
                    let wrong_output_write_path = self.problem_path.join("failing_test_wrong_output.out");
                    fs::write(official_output_write_path.clone(), correct_output).map_err(move |err| Error::IOError {
                        file: path_str(&official_output_write_path),
                        err,
                    })?;

                    if let RunResult::Ok(_, output) = &results[i + 1] {
                        fs::write(wrong_output_write_path.clone(), output).map_err(move |err| Error::IOError {
                            file: path_str(&wrong_output_write_path),
                            err,
                        })?;
                    } else if wrong_output_write_path.is_file() {
                        fs::remove_file(wrong_output_write_path.clone()).map_err(move |err| Error::IOError {
                            file: path_str(&wrong_output_write_path),
                            err,
                        })?;
                    }

                    fs::write(write_path.clone(), input).map_err(move |err| Error::IOError { file: path_str(&write_path), err })?;
                    return Err(Error::PartialSolutionFailsSubtask {
                        partial_number: sol_idx + 1,
                        subtask_number: subtask_idx + 1,
                        subtask_name: self.subtasks[subtask_idx].name.clone(),
                        partial_name: self.solutions[sol_idx].name.clone(),
                        verdict: if matches!(result, RunResult::Ok(_, _)) { "WA".to_owned() } else { result.to_display_string() },
                        gen_id: gen_idx + 1,
                    });
                }
            }
        }

        if bad_progs.is_empty() {
            return Ok(Some(correct_output));
        }

        // Run Bad Solutions to ensure they fail
        let bad_results_start = 1 + good_progs.len();
        for res in &results[bad_results_start..] {
            match res {
                RunResult::Ok(_, output) if (self.checker)(input, &correct_output, output) => {
                    // A bad solution passed this test! This test is not robust enough.
                    return Ok(None);
                }
                _ => {} // Bad solution failed as expected (TLE, Crash, or WA)
            }
        }
        Ok(Some(correct_output))
    }
}

#[cfg(test)]
mod trim_whitespace_tests {
    use super::trim_whitespace;

    #[test]
    fn collapses_runs_of_spaces() {
        assert_eq!(trim_whitespace("1   2     3"), "1 2 3\n");
    }

    #[test]
    fn a_run_containing_a_newline_becomes_a_newline() {
        assert_eq!(trim_whitespace("1 2 \n  3"), "1 2\n3\n");
        assert_eq!(trim_whitespace("1\n\n\n2"), "1\n2\n");
    }

    #[test]
    fn drops_leading_whitespace() {
        assert_eq!(trim_whitespace("   1 2"), "1 2\n");
        assert_eq!(trim_whitespace("\n\n1 2"), "1 2\n");
        assert_eq!(trim_whitespace("\t 1"), "1\n");
    }

    #[test]
    fn drops_trailing_whitespace_and_ensures_one_newline() {
        assert_eq!(trim_whitespace("1 2   "), "1 2\n");
        assert_eq!(trim_whitespace("1 2\n\n\n"), "1 2\n");
        assert_eq!(trim_whitespace("1 2\n"), "1 2\n");
    }

    #[test]
    fn normalises_windows_line_endings() {
        assert_eq!(trim_whitespace("1 2\r\n3 4\r\n"), "1 2\n3 4\n");
    }

    #[test]
    fn whitespace_only_input_becomes_a_single_newline() {
        assert_eq!(trim_whitespace(""), "\n");
        assert_eq!(trim_whitespace("   "), "\n");
        assert_eq!(trim_whitespace("\n\n"), "\n");
    }

    #[test]
    fn is_idempotent() {
        for input in ["  1  2 \n\n 3 ", "1\n2\n", "", "   ", "a\r\nb"] {
            let once = trim_whitespace(input);
            assert_eq!(trim_whitespace(&once), once, "not idempotent for {input:?}");
        }
    }
}
