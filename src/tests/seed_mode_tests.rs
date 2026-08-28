//! Tests for on-demand test generation: seed mode, the stubs it writes in place
//! of test data, and the server that turns one back into the test it stands for.
//!
//! The claim these have to hold up is a strong one — a test rebuilt from its
//! stub is the file a normal run would have written — so most of them work by
//! generating a task both ways and comparing the bytes.

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod seed_mode_tests {
    use crate::{Error, Mode, Subtask, Task};
    use log::LevelFilter;
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

    /// Every file a run left in the tests directory, as (name, contents), in name
    /// order.
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

    /// How many tests a run produced, counting a `.in` and its `.out` as one.
    fn num_tests(path: &Path) -> usize {
        test_files(path).len() / 2
    }

    /// The contents of the first input file whose name starts with `prefix`,
    /// which in seed mode is the stub that rebuilds it.
    fn input_file(path: &Path, prefix: &str) -> String {
        test_files(path)
            .into_iter()
            .find(|(name, _contents)| name.starts_with(prefix) && std::path::Path::new(name).extension().is_some_and(|extension| extension == "in"))
            .expect("there should be a test with that name")
            .1
    }

    /// Feeds stubs to a task's server, returning what it wrote and how the
    /// session ended: one that cannot be answered stops the server, and what it
    /// had written before that still matters.
    fn serve_raw(task: &Task<String>, requests: &str) -> (crate::Result<()>, String) {
        let mut output = Vec::new();
        let result = task.serve_io(&mut requests.as_bytes(), &mut output);
        (result, String::from_utf8(output).expect("what was served is UTF-8"))
    }

    /// Feeds stubs to a freshly built task, exactly as piping a test file into
    /// `--serve` would, and returns the raw bytes it answered with.
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

        // The first subtask's only test is n = 1, which the solution doubles.
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

    /// The whole point of the feature: piping a stub into the server gives back
    /// the file a normal run would have written, byte for byte.
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

    /// A task with no normalisation at all, whose test is the part most easily
    /// lost in transport: leading spaces, a tab, doubled blank lines and no
    /// trailing newline.
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

        // Not even a newline of its own: what comes back is the file, and this
        // file does not end in one.
        assert_eq!(served, awkward);
    }

    /// The whitespace setting changes the bytes of a test, so a task whose
    /// setting has moved since its stubs were written must not serve them.
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

    /// A run is reproducible: the same seed gives the same tests, a different one
    /// does not.
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

    /// Seed mode still hunts for counterexamples and still checks the partial
    /// solutions afterwards: keeping the tests as stubs must not drop the
    /// verification that makes the test data worth anything.
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
        assert_eq!(num_tests(dir.path()), NUM_TESTS + 2);
    }

    /// A stub written by hand carries no hash, so the server takes it on trust —
    /// and it is the `part` that decides whether the solution is run at all.
    #[test]
    fn a_stub_written_by_hand_is_served() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();

        // Subtask 0 has one generator, and it answers n = 1 whatever seed it gets.
        assert_eq!(serve(dir.path(), r#"{"subtask":0,"generator":0,"seed":"1234","part":"input"}"#), "1\n");
        assert_eq!(serve(dir.path(), r#"{"subtask":0,"generator":0,"seed":"1234","part":"output"}"#).trim(), "2");
    }

    /// With nothing framing a payload, a stub that cannot be answered has no way
    /// to say so in the stream. It ends the session instead, having written
    /// nothing, so a caller never mistakes an error for the test it asked for.
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
            // A good stub in front of it: what was already answered has to
            // survive the failure that follows.
            let requests = format!("{{\"subtask\":0,\"generator\":0,\"seed\":\"1\",\"part\":\"input\"}}\n{bad}");
            let (result, written) = serve_raw(&build_task(dir.path()), &requests);

            let err = result.unwrap_err();
            assert!(matches!(err, Error::InvalidStub { .. }), "{bad} gave {err}");
            assert_eq!(written, "1\n", "{bad} should not have added anything to the stream");
        }
    }

    /// If the generators change after the stubs are written, everything built
    /// from them is wrong. The server has to say so rather than serve a test
    /// that is not the one that was verified.
    #[test]
    fn a_changed_generator_is_detected() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).run_mode(Mode::Seeds).unwrap();
        let stub = input_file(dir.path(), "test.02");

        // Same task, same shape, but the second subtask's generator now produces
        // something else entirely.
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
    /// seed must not be written as a stub that promises it can.
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

        assert!(test_files(dir.path()).is_empty(), "nothing should be written for tests that cannot be rebuilt");
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

        assert_eq!(num_tests(dir.path()), 3);
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

        assert_eq!(num_tests(dir.path()), 3);
    }

    /// A faithful generator has to survive the check, however many times it is
    /// run - the check is worthless if it also rejects correct tasks.
    #[test]
    fn a_faithful_generator_passes_the_check() {
        let dir = TempDir::new().unwrap();
        build_task(dir.path()).with_reproducibility_checks(50).run_mode(Mode::Seeds).unwrap();

        assert_eq!(num_tests(dir.path()), NUM_TESTS);
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

    /// Nothing is written until everything has been verified, so a test set on
    /// disk always describes tests that were checked all the way through.
    #[test]
    fn a_failed_run_leaves_no_tests() {
        let dir = TempDir::new().unwrap();
        let task = build_task(dir.path()).with_partial_solution("always 2", PARTIAL, &[0, 1]);
        assert!(task.run_mode(Mode::Seeds).is_err());

        assert!(test_files(dir.path()).is_empty(), "a failed run should not leave tests behind");
        assert!(!dir.path().join("tests.zip").exists(), "a failed run should not leave an archive behind");
    }
}
