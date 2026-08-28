//! Tests for on-demand test generation: the seed manifest, seed mode and the
//! server that rebuilds tests from it.
//!
//! The claim these have to hold up is a strong one — a test served from a seed
//! is the same test a normal run would have written — so most of them work by
//! generating a task both ways and comparing the bytes.

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod seed_mode_tests {
    use crate::manifest::{Manifest, stable_hash};
    use crate::runner::cpp_runner::CppRunner;
    use crate::{Error, Mode, Subtask, Task};
    use log::LevelFilter;
    use serde_json::Value;
    use std::path::Path;
    use tempfile::TempDir;

    /// Doubles the number it is given.
    const SOLUTION: &str = "
        #include <iostream>
        int main() { long long n; std::cin >> n; std::cout << n * 2 << std::endl; }
    ";

    /// Always answers 2, so it is only right when the input is 1.
    const PARTIAL: &str = "
        #include <iostream>
        int main() { std::cout << 2 << std::endl; }
    ";

    /// How many tests [`build_task`] generates when no partial solution sends it
    /// looking for more.
    ///
    /// The second subtask draws from a range wide enough that its six tests are
    /// never duplicates of one another, so the count does not depend on luck.
    const NUM_TESTS: usize = 7;

    /// The task the tests generate.
    ///
    /// The first subtask is the one [`PARTIAL`] gets right; the second never
    /// contains n = 1, so [`PARTIAL`] fails every test in it.
    fn build_task(path: &Path) -> Task<String> {
        Task::new("Doubler", path)
            .with_debug_level(LevelFilter::Off)
            .with_solution_source(SOLUTION)
            .with_subtask(Subtask::new(30, "n = 1").with_test(1, |_rng| "1\n".to_owned()))
            .with_subtask(Subtask::new(70, "n <= 1000000000").with_test(6, |rng| format!("{}\n", rng.random_range(2..=1_000_000_000))))
    }

    /// Runs requests against a task and returns both what was written and how
    /// the session ended: a refused request stops the server, and what it had
    /// written before that still matters.
    fn serve_raw(task: &Task<String>, requests: &str) -> (crate::Result<()>, String) {
        let mut output = Vec::new();
        let result = task.serve_io(&mut requests.as_bytes(), &mut output);
        (result, String::from_utf8(output).expect("what was served is UTF-8"))
    }

    /// Runs requests against a freshly built task, exactly as a judge invoking
    /// `--serve` would, and returns the raw bytes it answered with.
    fn serve(path: &Path, requests: &str) -> String {
        let (result, written) = serve_raw(&build_task(path), requests);
        result.expect("the server should answer");
        written
    }

    fn read_manifest(path: &Path) -> Manifest {
        Manifest::read(&path.join("seeds.json")).unwrap()
    }

    #[test]
    fn files_mode_writes_a_manifest_beside_the_tests() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Files).unwrap();

        let manifest = read_manifest(dir.path());
        assert_eq!(manifest.task, "Doubler");
        assert_eq!(manifest.num_tests(), NUM_TESTS);
        assert_eq!(manifest.subtasks.len(), 2);
        assert_eq!(manifest.subtasks[0].points, 30);
        assert_eq!(manifest.subtasks[1].name, "n <= 1000000000");

        // Every recorded test names a file that is really there and really holds
        // what the manifest says it holds.
        for subtask in &manifest.subtasks {
            for test in &subtask.tests {
                let input = std::fs::read_to_string(dir.path().join("tests").join(&test.input_file)).unwrap();
                let output = std::fs::read_to_string(dir.path().join("tests").join(&test.output_file)).unwrap();
                assert_eq!(stable_hash(&input), test.input_hash, "{} does not match its recorded hash", test.input_file);
                assert_eq!(stable_hash(&output), test.output_hash, "{} does not match its recorded hash", test.output_file);
            }
        }
    }

    /// The whole point of the feature: a test served from its seed has to be the
    /// file a normal run wrote, byte for byte, input and output alike.
    #[test]
    fn a_served_test_is_byte_for_byte_the_file_that_was_written() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Files).unwrap();
        let manifest = read_manifest(dir.path());

        let entries = manifest
            .subtasks
            .iter()
            .flat_map(|subtask| subtask.tests.iter().map(move |test| (subtask.index, test)))
            .collect::<Vec<_>>();

        for part in ["input", "output"] {
            let requests = entries
                .iter()
                .map(|(subtask, test)| format!(r#"{{"command":"test","subtask":{subtask},"test":{},"part":"{part}"}}"#, test.index_in_subtask))
                .collect::<Vec<_>>()
                .join("\n");

            // Nothing separates one answer from the next, so a whole session of
            // them is the files themselves laid end to end.
            let from_disk = entries
                .iter()
                .map(|(_subtask, test)| {
                    let file = if part == "input" { &test.input_file } else { &test.output_file };
                    std::fs::read_to_string(dir.path().join("tests").join(file)).unwrap()
                })
                .collect::<String>();

            assert_eq!(serve(dir.path(), &requests), from_disk, "the served {part}s are not what was written to disk");
        }
    }

    /// Whitespace is the part of a test most easily lost in transport, so it gets
    /// its own check with a task that does no normalisation at all.
    #[test]
    fn whitespace_survives_being_served() {
        let dir = TempDir::new().unwrap();
        // Leading spaces, a tab, doubled blank lines and no trailing newline: a
        // normalising path would quietly change every one of them.
        let awkward = "  3 \t\n\n\n 1   2     3";
        let build = |path: &Path| {
            Task::new("Whitespace", path)
                .with_debug_level(LevelFilter::Off)
                .trim_whitespace(false)
                .with_solution_source(
                    "
                    #include <iostream>
                    int main() { int n; std::cin >> n; std::cout << n; }
                ",
                )
                .with_subtask(Subtask::new(100, "one test").with_test(1, move |_rng| awkward.to_owned()))
        };

        build(dir.path()).run_mode(Mode::Files).unwrap();
        let manifest = read_manifest(dir.path());
        let test = &manifest.subtasks[0].tests[0];

        let from_disk = std::fs::read_to_string(dir.path().join("tests").join(&test.input_file)).unwrap();
        assert_eq!(from_disk, awkward, "the file itself should hold the untouched input");

        let (result, served) = serve_raw(&build(dir.path()), r#"{"subtask":0,"test":0,"part":"input"}"#);
        result.unwrap();

        // Not even a newline of its own: what comes back is the file and nothing
        // more, and this file does not end in one.
        assert_eq!(served, awkward);
    }

    #[test]
    fn seed_mode_writes_no_test_files() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        assert!(!dir.path().join("tests").exists(), "seed mode should not create a tests directory");
        assert!(!dir.path().join("tests.zip").exists(), "seed mode should not create an archive");
        assert_eq!(read_manifest(dir.path()).num_tests(), NUM_TESTS);
    }

    /// Seed mode is the same run as file mode with the writing left out, so the
    /// two have to describe exactly the same tests.
    #[test]
    fn seed_mode_and_files_mode_produce_the_same_tests() {
        let files_dir = TempDir::new().unwrap();
        let seeds_dir = TempDir::new().unwrap();

        build_task(files_dir.path()).run_mode(Mode::Files).unwrap();
        build_task(seeds_dir.path()).run_mode(Mode::Seeds).unwrap();

        let from_files = read_manifest(files_dir.path());
        let from_seeds = read_manifest(seeds_dir.path());
        assert_eq!(from_files.subtasks, from_seeds.subtasks);
        assert_eq!(from_files.seed, from_seeds.seed);
    }

    /// A run is reproducible: the same seed gives the same tests, a different one
    /// does not.
    #[test]
    fn the_seed_decides_the_tests() {
        let run = |seed: u64| {
            let dir = TempDir::new().unwrap();
            build_task(dir.path()).with_seed(seed).run_mode(Mode::Seeds).unwrap();
            read_manifest(dir.path()).subtasks
        };

        assert_eq!(run(1234), run(1234), "the same seed produced different tests");
        assert_ne!(run(1234), run(5678), "two seeds produced identical tests");
    }

    /// Seed mode still hunts for counterexamples and still checks the partial
    /// solutions afterwards: dropping the files must not drop the verification
    /// that makes the test data worth anything.
    #[test]
    fn seed_mode_still_verifies_partial_solutions() {
        let dir = TempDir::new().unwrap();
        // The partial solution only survives where n is 1, so declaring that it
        // passes the second subtask too has to be caught.
        let task = build_task(dir.path()).with_partial_solution("always 2", PARTIAL, &[0, 1]);

        let err = task.run_mode(Mode::Seeds).unwrap_err();
        assert!(
            matches!(err, Error::PartialSolutionPassesExtraSubtask { .. } | Error::PartialSolutionFailsSubtask { .. }),
            "expected the partial solution to be caught, got {err}"
        );
    }

    #[test]
    fn seed_mode_accepts_a_correctly_declared_partial_solution() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path())
            .with_partial_solution("always 2", PARTIAL, &[0])
            .with_min_failures(2)
            .run_mode(Mode::Seeds)
            .unwrap();

        // The two extra tests are the counterexamples that were hunted down for
        // the second subtask, which the partial solution is declared to fail.
        assert_eq!(read_manifest(dir.path()).num_tests(), NUM_TESTS + 2);
    }

    #[test]
    fn the_server_describes_itself() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        // The one answer that is still JSON, because it describes the manifest
        // rather than carrying a test.
        let response = serve(dir.path(), r#"{"command":"info"}"#);
        let info: Value = serde_json::from_str(response.trim()).unwrap();
        assert_eq!(info["ok"], Value::Bool(true));
        assert_eq!(info["task"], "Doubler");
        assert_eq!(info["num_tests"], NUM_TESTS);
        assert_eq!(info["subtasks"][0]["num_tests"], 1);
        assert_eq!(info["subtasks"][1]["points"], 70);
    }

    /// A response carries one half of a test, so every request has to name the
    /// half it wants.
    #[test]
    fn each_half_of_a_test_is_asked_for_separately() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        // The first subtask's only test is n = 1, and the solution doubles it.
        assert_eq!(serve(dir.path(), r#"{"subtask":0,"test":0,"part":"input"}"#), "1\n");
        assert_eq!(serve(dir.path(), r#"{"subtask":0,"test":0,"part":"output"}"#).trim(), "2");

        let (result, written) = serve_raw(&build_task(dir.path()), r#"{"subtask":0,"test":0}"#);
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InvalidArguments { .. }), "got {err}");
        assert!(err.to_string().contains("part"), "{err}");
        assert!(written.is_empty(), "nothing should have been served");
    }

    #[test]
    fn a_test_can_be_asked_for_by_seed() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();
        let manifest = read_manifest(dir.path());
        let recorded = &manifest.subtasks[1].tests[0];

        let request = format!(r#"{{"command":"seed","subtask":1,"generator":{},"seed":"{:016x}","part":"input"}}"#, recorded.generator, recorded.seed);

        assert_eq!(stable_hash(&serve(dir.path(), &request)), recorded.input_hash);
    }

    /// With nothing framing a payload, a request that cannot be answered has no
    /// way to say so in the stream. It ends the session instead, having written
    /// nothing, so a caller never mistakes an error for the test it asked for.
    #[test]
    fn a_request_that_cannot_be_answered_ends_the_session() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        for bad in [
            "not json at all",
            r#"{"command":"nonsense"}"#,
            r#"{"command":"test","subtask":0,"part":"input"}"#,
            r#"{"command":"test","subtask":0,"test":0,"part":"both"}"#,
            r#"{"command":"test","subtask":0,"test":999,"part":"input"}"#,
            r#"{"command":"test","subtask":99,"test":0,"part":"input"}"#,
        ] {
            // A good request in front of it: what was already answered has to
            // survive the failure that follows.
            let requests = format!("{{\"subtask\":0,\"test\":0,\"part\":\"input\"}}\n{bad}");
            let (result, written) = serve_raw(&build_task(dir.path()), &requests);

            let err = result.unwrap_err();
            assert!(matches!(err, Error::InvalidArguments { .. }), "{bad} gave {err}");
            assert_eq!(written, "1\n", "{bad} should not have added anything to the stream");
        }
    }

    #[test]
    fn quit_ends_the_session() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        let served = serve(
            dir.path(),
            concat!(
                "\n",
                r#"{"command":"test","subtask":0,"test":0,"part":"input"}"#,
                "\n",
                r#"{"command":"quit"}"#,
                "\n",
                r#"{"command":"test","subtask":1,"test":0,"part":"input"}"#
            ),
        );
        // The blank line is skipped rather than answered, and the request after
        // quit is not answered at all.
        assert_eq!(served, "1\n", "nothing after quit should be answered");
    }

    /// Serving from a manifest that belongs to another task would hand out test
    /// data for the wrong problem.
    #[test]
    fn a_manifest_from_another_task_is_refused() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        let other = Task::new("Something Else", dir.path())
            .with_debug_level(LevelFilter::Off)
            .with_solution_source(SOLUTION)
            .with_subtask(Subtask::new(30, "n = 1").with_test(1, |_rng| "1\n".to_owned()))
            .with_subtask(Subtask::new(70, "other").with_test(6, |_rng| "1\n".to_owned()));

        let err = other.serve_io(&mut std::io::empty(), &mut Vec::new()).unwrap_err();
        assert!(matches!(err, Error::ManifestMismatch { .. }), "got {err}");
        assert!(err.to_string().contains("Something Else"), "{err}");
    }

    /// If the generators change after a manifest is written, everything built
    /// from that manifest is wrong. The server has to say so rather than serve a
    /// test that is not the one that was recorded.
    #[test]
    fn a_changed_generator_is_detected() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        // Same task, same shape, but the second subtask's generator now produces
        // something else entirely.
        let changed = Task::new("Doubler", dir.path())
            .with_debug_level(LevelFilter::Off)
            .with_solution_source(SOLUTION)
            .with_subtask(Subtask::new(30, "n = 1").with_test(1, |_rng| "1\n".to_owned()))
            .with_subtask(Subtask::new(70, "n <= 1000000000").with_test(6, |_rng| "777\n".to_owned()));

        let (result, written) = serve_raw(&changed, r#"{"subtask":1,"test":0,"part":"input"}"#);

        let err = result.unwrap_err();
        assert!(matches!(err, Error::ManifestMismatch { .. }), "got {err}");
        assert!(err.to_string().contains("no longer produces"), "{err}");
        assert!(written.is_empty(), "a test that does not match the manifest must not reach the caller");
    }

    /// A subtask that gained or lost generators is the same kind of drift, and
    /// the manifest can name a generator that is no longer there at all.
    #[test]
    fn a_manifest_with_the_wrong_number_of_subtasks_is_refused() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        let shorter = Task::new("Doubler", dir.path())
            .with_debug_level(LevelFilter::Off)
            .with_solution_source(SOLUTION)
            .with_subtask(Subtask::new(30, "n = 1").with_test(2, |_rng| "1\n".to_owned()));

        let err = shorter.serve_io(&mut std::io::empty(), &mut Vec::new()).unwrap_err();
        assert!(matches!(err, Error::ManifestMismatch { .. }), "got {err}");
    }

    /// The manifest records how the tests were normalised, and serving them under
    /// a different setting would produce different bytes.
    #[test]
    fn a_manifest_written_with_other_whitespace_settings_is_refused() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        let untrimmed = build_task(dir.path()).trim_whitespace(false);
        let err = untrimmed.serve_io(&mut std::io::empty(), &mut Vec::new()).unwrap_err();
        assert!(err.to_string().contains("trim_whitespace"), "{err}");
    }

    /// Regenerating a test in process, without the protocol in between, has to
    /// give back what was generated the first time.
    #[test]
    fn a_test_can_be_rebuilt_from_its_seed() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Files).unwrap();
        let manifest = read_manifest(dir.path());

        let task = build_task(dir.path());
        let mut cpp_runner = CppRunner::new(&dir.path().join("build")).unwrap();
        let solution_handle = cpp_runner.add_program(SOLUTION).unwrap();

        for subtask in &manifest.subtasks {
            for recorded in &subtask.tests {
                let rebuilt = task.regenerate_test(subtask.index, recorded.generator, recorded.seed, &mut cpp_runner, solution_handle).unwrap();
                assert_eq!(stable_hash(&rebuilt.input), recorded.input_hash);
                assert_eq!(stable_hash(&rebuilt.output), recorded.output_hash);
            }
        }
    }

    #[test]
    fn a_missing_manifest_is_reported() {
        let dir = TempDir::new().unwrap();
        let err = build_task(dir.path()).serve_io(&mut std::io::empty(), &mut Vec::new()).unwrap_err();
        assert!(matches!(err, Error::IOError { .. }), "got {err}");
    }

    #[test]
    fn a_corrupt_manifest_is_reported() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();
        std::fs::write(dir.path().join("seeds.json"), "{ this is not json").unwrap();

        let err = build_task(dir.path()).serve_io(&mut std::io::empty(), &mut Vec::new()).unwrap_err();
        assert!(matches!(err, Error::InvalidManifest { .. }), "got {err}");
    }

    /// A generator that ignores the `Rng` it is given and counts instead, so no
    /// two calls agree. Nothing stops one being written, which is why seed mode
    /// goes looking for it.
    fn unfaithful_task(path: &Path) -> Task<String> {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicI64, Ordering};

        let counter = Arc::new(AtomicI64::new(1));
        Task::new("Unfaithful", path)
            .with_debug_level(LevelFilter::Off)
            .with_solution_source(SOLUTION)
            .with_subtask(Subtask::new(100, "counted").with_test(3, move |_rng| format!("{}\n", counter.fetch_add(1, Ordering::SeqCst))))
    }

    /// The check seed mode exists for: a test that cannot be rebuilt from its
    /// seed must not be recorded as though it could.
    #[test]
    fn seed_mode_catches_a_generator_that_is_not_reproducible() {
        let dir = TempDir::new().unwrap();
        let err = unfaithful_task(dir.path()).run_mode(Mode::Seeds).unwrap_err();

        let Error::GeneratorNotReproducible {
            subtask_number,
            gen_id,
            attempts,
            details,
            ..
        } = &err
        else {
            unreachable!("expected an unreproducible generator, got {err}")
        };
        assert_eq!(*subtask_number, 1);
        assert_eq!(*gen_id, 1);
        assert_eq!(*attempts, crate::DEFAULT_REPRODUCIBILITY_CHECKS);
        assert!(details.contains("differ"), "{details}");

        assert!(!dir.path().join("seeds.json").exists(), "a manifest must not be written for tests that cannot be rebuilt");
    }

    /// The error has to say what to do about it, because the cause is always the
    /// same mistake in the generator.
    #[test]
    fn the_error_explains_what_went_wrong() {
        let dir = TempDir::new().unwrap();
        let message = unfaithful_task(dir.path()).run_mode(Mode::Seeds).unwrap_err().to_string();

        assert!(message.contains("not reproducible"), "{message}");
        assert!(message.contains("Rng it is given"), "{message}");
        assert!(message.contains("seed 0x"), "{message}");
    }

    /// File mode keeps the tests themselves, so it does not pay for the check
    /// unless it is asked to.
    #[test]
    fn files_mode_does_not_check_by_default() {
        let dir = TempDir::new().unwrap();
        unfaithful_task(dir.path()).run_mode(Mode::Files).unwrap();

        assert_eq!(read_manifest(dir.path()).num_tests(), 3);
    }

    #[test]
    fn files_mode_checks_when_asked_to() {
        let dir = TempDir::new().unwrap();
        let err = unfaithful_task(dir.path()).with_reproducibility_checks(4).run_mode(Mode::Files).unwrap_err();

        let Error::GeneratorNotReproducible { attempts, .. } = &err else {
            unreachable!("expected an unreproducible generator, got {err}")
        };
        assert_eq!(*attempts, 4);
    }

    #[test]
    fn the_check_can_be_turned_off() {
        let dir = TempDir::new().unwrap();
        unfaithful_task(dir.path()).with_reproducibility_checks(0).run_mode(Mode::Seeds).unwrap();

        assert_eq!(read_manifest(dir.path()).num_tests(), 3);
    }

    /// A faithful generator has to survive the check, however many times it is
    /// run - the check is worthless if it also rejects correct tasks.
    #[test]
    fn a_faithful_generator_passes_the_check() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).with_reproducibility_checks(50).run_mode(Mode::Seeds).unwrap();

        assert_eq!(read_manifest(dir.path()).num_tests(), NUM_TESTS);
    }

    /// Only the tests that were kept are rebuilt, and each of them exactly as
    /// many times as was asked for.
    #[test]
    fn the_check_rebuilds_every_kept_test_the_requested_number_of_times() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let dir = TempDir::new().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);

        // Faithful - the value comes from the rng - but it counts how often it is
        // asked for a test.
        let task = Task::new("Counted", dir.path())
            .with_debug_level(LevelFilter::Off)
            .with_solution_source(SOLUTION)
            .with_reproducibility_checks(7)
            .with_subtask(Subtask::new(100, "counted").with_test(4, move |rng| {
                counted.fetch_add(1, Ordering::SeqCst);
                format!("{}\n", rng.random_range(1..=1_000_000_000))
            }));

        task.run_mode(Mode::Seeds).unwrap();

        // Four calls to generate the tests, then seven rebuilds of each of them.
        assert_eq!(calls.load(Ordering::SeqCst), 4 + 4 * 7);
    }

    /// Seed mode writes the manifest only once everything has been verified, so a
    /// manifest on disk always describes tests that were checked all the way
    /// through.
    #[test]
    fn a_failed_run_leaves_no_manifest() {
        let dir = TempDir::new().unwrap();
        let task = build_task(dir.path()).with_partial_solution("always 2", PARTIAL, &[0, 1]);
        assert!(task.run_mode(Mode::Seeds).is_err());

        assert!(!dir.path().join("seeds.json").exists(), "a failed run should not leave a manifest behind");
    }
}
