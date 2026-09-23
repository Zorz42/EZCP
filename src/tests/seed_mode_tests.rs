//! Seed mode and serving. Most tests check that a test rebuilt from its stub
//! is byte for byte the file a normal run writes.

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod seed_mode_tests {
    use crate::{Error, Mode, Subtask, Task};
    use log::LevelFilter;
    use std::path::Path;
    use tempfile::TempDir;

    const SOLUTION: &str = "
        #include <iostream>
        int main() { long long n; std::cin >> n; std::cout << n * 2 << std::endl; }
    ";

    /// Only right when n is 1.
    const PARTIAL: &str = "
        #include <iostream>
        int main() { std::cout << 2 << std::endl; }
    ";

    /// Tests in [`build_task`] when nothing is hunted for. The range is wide enough
    /// that duplicates never lower the count.
    const NUM_TESTS: usize = 7;

    /// [`PARTIAL`] passes the first subtask and fails every test of the second.
    fn build_task(path: &Path) -> Task<String> {
        Task::new("Doubler", path)
            .with_debug_level(LevelFilter::Off)
            .with_solution_source(SOLUTION)
            .with_subtask(Subtask::new(30, "n = 1").with_test(1, |_rng| "1\n".to_owned()))
            .with_subtask(Subtask::new(70, "n <= 1000000000").with_test(6, |rng| format!("{}\n", rng.random_range(2..=1_000_000_000))))
    }

    /// `(name, contents)` of every test file, sorted by name.
    fn test_files(path: &Path) -> Vec<(String, String)> {
        let mut files = std::fs::read_dir(path.join("tests"))
            .expect("a run should leave a tests directory")
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name().to_string_lossy().into_owned(), std::fs::read_to_string(entry.path()).unwrap())
            })
            .collect::<Vec<_>>();
        files.sort();
        files
    }

    fn num_tests(path: &Path) -> usize {
        test_files(path).len() / 2
    }

    fn input_file(path: &Path, prefix: &str) -> String {
        test_files(path)
            .into_iter()
            .find(|(name, _contents)| name.starts_with(prefix) && Path::new(name).extension().is_some_and(|extension| extension == "in"))
            .expect("there should be a test with that name")
            .1
    }

    /// Also returns what was written when the session ended with an error.
    fn serve_raw(task: &Task<String>, requests: &str) -> (crate::Result<()>, String) {
        let mut output = Vec::new();
        let result = task.serve_io(&mut requests.as_bytes(), &mut output);
        (result, String::from_utf8(output).expect("what was served is UTF-8"))
    }

    fn serve(path: &Path, requests: &str) -> String {
        let (result, written) = serve_raw(&build_task(path), requests);
        result.expect("the server should answer");
        written
    }

    #[test]
    fn files_mode_writes_the_tests_themselves() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Files).unwrap();

        assert_eq!(num_tests(dir.path()), NUM_TESTS);
        assert!(dir.path().join("tests.zip").exists());

        let files = test_files(dir.path());
        assert_eq!(files[0].1, "1\n");
        assert_eq!(files[1].1.trim(), "2");
    }

    #[test]
    fn seed_mode_writes_the_same_test_set_as_stubs() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        assert_eq!(num_tests(dir.path()), NUM_TESTS);
        assert!(dir.path().join("tests.zip").exists(), "stubs are archived like any other test set");
        assert!(!dir.path().join("seeds.json").exists(), "a stub carries everything, so there is no manifest");

        for (name, contents) in test_files(dir.path()) {
            assert_eq!(contents.lines().count(), 1, "{name} should hold one line, not {contents:?}");
            assert!(contents.len() < 200, "{name} holds {} bytes; a stub is a recipe, not a test", contents.len());
            assert!(contents.contains("\"seed\""), "{name} does not look like a stub: {contents}");
        }
    }

    #[test]
    fn a_stub_rebuilds_the_file_a_normal_run_wrote() {
        let files_dir = TempDir::new().unwrap();
        let seeds_dir = TempDir::new().unwrap();
        build_task(files_dir.path()).run_mode(Mode::Files).unwrap();
        build_task(seeds_dir.path()).run_mode(Mode::Seeds).unwrap();

        let written = test_files(files_dir.path());
        let stubs = test_files(seeds_dir.path());
        let names = |files: &[(String, String)]| files.iter().map(|(name, _contents)| name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&written), names(&stubs), "the two modes should name their tests the same way");

        for ((name, contents), (_same_name, stub)) in written.iter().zip(&stubs) {
            assert_eq!(serve(seeds_dir.path(), stub), *contents, "the stub for {name} did not rebuild it");
        }
    }

    /// A test whose whitespace any normalisation would change.
    fn whitespace_task(path: &Path, trim: bool) -> Task<String> {
        Task::new("Whitespace", path)
            .with_debug_level(LevelFilter::Off)
            .trim_whitespace(trim)
            .with_solution_source(
                "
                #include <iostream>
                int main() { int n; std::cin >> n; std::cout << n; }
            ",
            )
            .with_subtask(Subtask::new(100, "one test").with_test(1, |_rng| "  3 \t\n\n\n 1   2     3".to_owned()))
    }

    #[test]
    fn whitespace_survives_being_rebuilt() {
        let awkward = "  3 \t\n\n\n 1   2     3";
        let files_dir = TempDir::new().unwrap();
        let seeds_dir = TempDir::new().unwrap();

        whitespace_task(files_dir.path(), false).run_mode(Mode::Files).unwrap();
        whitespace_task(seeds_dir.path(), false).run_mode(Mode::Seeds).unwrap();
        assert_eq!(input_file(files_dir.path(), "test"), awkward, "the file itself should hold the untouched input");

        let stub = input_file(seeds_dir.path(), "test");
        let (result, served) = serve_raw(&whitespace_task(seeds_dir.path(), false), &stub);
        result.unwrap();

        // Byte for byte, so without a trailing newline either.
        assert_eq!(served, awkward);
    }

    #[test]
    fn a_changed_whitespace_setting_is_refused() {
        let dir = TempDir::new().unwrap();
        whitespace_task(dir.path(), false).run_mode(Mode::Seeds).unwrap();
        let stub = input_file(dir.path(), "test");

        let (result, written) = serve_raw(&whitespace_task(dir.path(), true), &stub);
        let err = result.unwrap_err();
        assert!(matches!(err, Error::StubMismatch { .. }), "got {err}");
        assert!(written.is_empty(), "a test that does not match its stub must not reach the caller");
    }

    #[test]
    fn the_seed_decides_the_tests() {
        let run = |seed: u64| {
            let dir = TempDir::new().unwrap();
            build_task(dir.path()).with_seed(seed).run_mode(Mode::Seeds).unwrap();
            test_files(dir.path())
        };

        assert_eq!(run(1234), run(1234), "the same seed produced different tests");
        assert_ne!(run(1234), run(5678), "two seeds produced identical tests");
    }

    #[test]
    fn seed_mode_still_verifies_partial_solutions() {
        let dir = TempDir::new().unwrap();
        let task = build_task(dir.path()).with_partial_solution("always 2", PARTIAL, &[0, 1]);

        let err = task.run_mode(Mode::Seeds).unwrap_err();
        assert!(
            matches!(err, Error::PartialSolutionPassesExtraSubtask { .. } | Error::PartialSolutionFailsSubtask { .. }),
            "expected the partial solution to be caught, got {err}"
        );
        assert!(test_files(dir.path()).is_empty(), "a failed run should not leave tests behind");
        assert!(!dir.path().join("tests.zip").exists(), "a failed run should not leave an archive behind");
    }

    #[test]
    fn a_partial_solution_the_initial_tests_already_break_costs_no_extra_tests() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path())
            .with_partial_solution("always 2", PARTIAL, &[0])
            .with_min_failures(2)
            .run_mode(Mode::Seeds)
            .unwrap();

        assert_eq!(num_tests(dir.path()), NUM_TESTS);
    }

    #[test]
    fn a_stub_written_by_hand_is_served() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        // That generator ignores its seed and always gives n = 1.
        assert_eq!(serve(dir.path(), r#"{"subtask":0,"generator":0,"seed":"1234","part":"input"}"#), "1\n");
        assert_eq!(serve(dir.path(), r#"{"subtask":0,"generator":0,"seed":"1234","part":"output"}"#).trim(), "2");
    }

    #[test]
    fn serving_inputs_compiles_nothing() {
        let dir = TempDir::new().unwrap();

        let requests = [
            r#"{"subtask":0,"generator":0,"seed":"1234","part":"input"}"#,
            r#"{"subtask":1,"generator":0,"seed":"5678","part":"input"}"#,
        ]
        .join("\n");
        let served = serve(dir.path(), &requests);

        assert!(served.starts_with("1\n"), "{served:?}");
        assert!(!dir.path().join("build").exists(), "serving inputs should not have compiled anything");
    }

    /// Unframed output cannot carry an error, so the session ends instead.
    #[test]
    fn a_stub_that_cannot_be_answered_ends_the_session() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        for bad in [
            "not a stub at all",
            r#"{"generator":0,"seed":"1","part":"input"}"#,
            r#"{"subtask":0,"seed":"1","part":"input"}"#,
            r#"{"subtask":0,"generator":0,"part":"input"}"#,
            r#"{"subtask":0,"generator":0,"seed":"1"}"#,
            r#"{"subtask":0,"generator":0,"seed":"1","part":"both"}"#,
            r#"{"subtask":9,"generator":0,"seed":"1","part":"input"}"#,
            r#"{"subtask":0,"generator":9,"seed":"1","part":"input"}"#,
        ] {
            // What was answered before the bad stub must remain.
            let requests = format!("{{\"subtask\":0,\"generator\":0,\"seed\":\"1\",\"part\":\"input\"}}\n{bad}");
            let (result, written) = serve_raw(&build_task(dir.path()), &requests);

            let err = result.unwrap_err();
            assert!(matches!(err, Error::InvalidStub { .. }), "{bad} gave {err}");
            assert_eq!(written, "1\n", "{bad} should not have added anything to the stream");
        }
    }

    #[test]
    fn a_changed_generator_is_detected() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();
        let stub = input_file(dir.path(), "test.02");

        let changed = Task::new("Doubler", dir.path())
            .with_debug_level(LevelFilter::Off)
            .with_solution_source(SOLUTION)
            .with_subtask(Subtask::new(30, "n = 1").with_test(1, |_rng| "1\n".to_owned()))
            .with_subtask(Subtask::new(70, "n <= 1000000000").with_test(6, |_rng| "777\n".to_owned()));

        let (result, written) = serve_raw(&changed, &stub);
        let err = result.unwrap_err();
        assert!(matches!(err, Error::StubMismatch { .. }), "got {err}");
        assert!(err.to_string().contains("no longer produces"), "{err}");
        assert!(written.is_empty(), "a test that does not match its stub must not reach the caller");
    }

    /// Ignores its `Rng`, so no two calls agree.
    fn unfaithful_task(path: &Path) -> Task<String> {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicI64, Ordering};

        let counter = Arc::new(AtomicI64::new(1));
        Task::new("Unfaithful", path)
            .with_debug_level(LevelFilter::Off)
            .with_solution_source(SOLUTION)
            .with_subtask(Subtask::new(100, "counted").with_test(3, move |_rng| format!("{}\n", counter.fetch_add(1, Ordering::SeqCst))))
    }

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
        let message = err.to_string();
        for part in ["not reproducible", "Rng it is given", "seed 0x"] {
            assert!(message.contains(part), "{message}");
        }

        assert!(test_files(dir.path()).is_empty(), "nothing should be written for tests that cannot be rebuilt");
    }

    #[test]
    fn the_check_follows_its_setting() {
        let dir = TempDir::new().unwrap();
        unfaithful_task(dir.path()).run_mode(Mode::Files).unwrap();
        assert_eq!(num_tests(dir.path()), 3, "files mode does not check by default");

        let err = unfaithful_task(dir.path()).with_reproducibility_checks(4).run_mode(Mode::Files).unwrap_err();
        assert!(matches!(err, Error::GeneratorNotReproducible { attempts: 4, .. }), "got {err}");

        unfaithful_task(dir.path()).with_reproducibility_checks(0).run_mode(Mode::Seeds).unwrap();
        assert_eq!(num_tests(dir.path()), 3, "the check can be turned off");
    }

    #[test]
    fn a_faithful_generator_passes_the_check() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).with_reproducibility_checks(50).run_mode(Mode::Seeds).unwrap();

        assert_eq!(num_tests(dir.path()), NUM_TESTS);
    }

    #[test]
    fn the_check_rebuilds_every_kept_test_the_requested_number_of_times() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let dir = TempDir::new().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);

        // Uses its rng, but counts the calls.
        let task = Task::new("Counted", dir.path())
            .with_debug_level(LevelFilter::Off)
            .with_solution_source(SOLUTION)
            .with_reproducibility_checks(7)
            .with_subtask(Subtask::new(100, "counted").with_test(4, move |rng| {
                counted.fetch_add(1, Ordering::SeqCst);
                format!("{}\n", rng.random_range(1..=1_000_000_000))
            }));

        task.run_mode(Mode::Seeds).unwrap();

        // Four to generate, then seven rebuilds of each.
        assert_eq!(calls.load(Ordering::SeqCst), 4 + 4 * 7);
    }
}
