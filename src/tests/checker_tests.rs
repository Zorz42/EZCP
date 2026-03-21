/// Tests for the custom checker feature (`with_checker` / `diff_checker`).
///
/// The checker function signature is `fn(&str, &str, &str) -> bool` where
/// arguments are (test_input, correct_output, program_output) and the return
/// value is `true` when the program output is ACCEPTED (correct), `false`
/// when it is REJECTED (wrong answer).
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod checker_tests {
    use crate::tests::generic_tests::generic_tests::Test;
    use crate::{Error, Subtask, Task};
    use log::LevelFilter;
    use tempfile::TempDir;

    // ─── helpers ────────────────────────────────────────────────────────────────

    /// A checker that accepts any output (ignores everything – every answer is
    /// treated as correct).
    fn always_accept(_input: &str, _correct: &str, _program: &str) -> bool {
        true
    }

    /// A checker that rejects every output (every answer is treated as wrong).
    fn always_reject(_input: &str, _correct: &str, _program: &str) -> bool {
        false
    }

    /// A checker that compares outputs as exact byte-for-byte strings (stricter
    /// than the default `diff_checker` which ignores surrounding whitespace).
    fn exact_checker(_input: &str, correct: &str, program: &str) -> bool {
        correct == program
    }

    /// A checker for problems where any non-negative integer is accepted as
    /// output (represents "any valid answer" semantics).
    fn any_nonneg_integer_checker(_input: &str, _correct: &str, program: &str) -> bool {
        program.trim().parse::<i64>().is_ok_and(|v| v >= 0)
    }

    // ─── unit tests for diff_checker (the default) ───────────────────────────

    /// The default checker is exposed only through `Task`, so we test its
    /// behaviour end-to-end: a solution that emits a trailing newline should
    /// be accepted because `diff_checker` is whitespace-normalised.
    #[test]
    fn default_checker_accepts_trailing_whitespace() {
        let mut task = Test::new();

        // Main solution prints "42\n"
        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 42 << "\n"; return 0; }
        "#;

        // Partial solution prints "42\n\n" (extra newline) – should be OK with diff_checker.
        let good_partial = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 42 << "\n\n"; return 0; }
        "#;

        task.task = task
            .task
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new("only subtask").with_test(3, || "".to_owned()))
            // This solution passes the only subtask (extra whitespace is fine).
            .with_partial_solution(good_partial, &[0]);

        task.test(); // must not error
    }

    #[test]
    fn default_checker_rejects_different_output() {
        let mut task = Test::new();

        // Main solution always prints "1".
        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 1 << "\n"; return 0; }
        "#;

        // Bad solution prints "2" — should be found wrong on every test.
        let bad_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 2 << "\n"; return 0; }
        "#;

        task.task = task
            .task
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new("only subtask").with_test(3, || "".to_owned()))
            .with_partial_solution(bad_solution, &[]); // fails all subtasks

        task.test(); // must succeed (bad solution is rejected as expected)
    }

    // ─── with_checker builder ────────────────────────────────────────────────

    #[test]
    fn with_checker_returns_self_for_chaining() {
        // Verify the builder method compiles and produces a valid Task.
        let tempdir = TempDir::new().unwrap();
        let _task = Task::new("chain test", tempdir.path())
            .with_debug_level(LevelFilter::Off)
            .with_checker(always_accept)
            .with_solution_source("int main() { return 0; }");
        // Just verifying it compiles and doesn't panic.
    }

    // ─── always_accept checker ───────────────────────────────────────────────

    /// When the checker accepts every output, a "bad" solution whose output
    /// differs from the main solution's output will always be considered
    /// correct by the checker.  That means every test looks non-robust (the
    /// bad solution appears to pass), so the task cannot accumulate enough
    /// robust tests and will stop after `max_tries` without erroring – it
    /// just logs a warning and writes 0 supplemental tests.
    ///
    /// The task itself should still *succeed* (return `Ok(())`); it only
    /// warns that not enough robust tests were found.
    #[test]
    fn always_accept_checker_with_bad_solution() {
        let tempdir = TempDir::new().unwrap();
        let task_path = tempdir.path().join("always_accept_bad");

        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 1 << "\n"; return 0; }
        "#;

        // This bad solution always produces the WRONG answer from the test
        // author's perspective, but our checker accepts everything.
        let bad_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 999 << "\n"; return 0; }
        "#;

        let task = Task::new("always_accept_bad", &task_path)
            .with_debug_level(LevelFilter::Off)
            .with_checker(always_accept)
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new("only subtask").with_test(2, || "".to_owned()))
            // The bad solution is supposed to fail the subtask, but our checker
            // never rejects it – so no robust test can be found.
            .with_partial_solution(bad_solution, &[])
            .with_min_failures(1)
            .with_max_tries(10);

        // Task should complete without returning an Err – it just warns.
        task.run().unwrap();
    }

    /// When the checker accepts every output, a partial solution that is
    /// declared as "good" (passes all subtasks) and produces different output
    /// should still be accepted – because the checker says it's fine.
    #[test]
    fn always_accept_checker_good_solution_any_output() {
        let tempdir = TempDir::new().unwrap();
        let task_path = tempdir.path().join("always_accept_good");

        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 42 << "\n"; return 0; }
        "#;

        // Good partial: produces a different number but checker doesn't care.
        let good_partial = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 0 << "\n"; return 0; }
        "#;

        let task = Task::new("always_accept_good", &task_path)
            .with_debug_level(LevelFilter::Off)
            .with_checker(always_accept)
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new("only subtask").with_test(2, || "".to_owned()))
            .with_partial_solution(good_partial, &[0]); // declared to pass subtask 0

        task.run().unwrap();
    }

    // ─── always_reject checker ───────────────────────────────────────────────

    /// When the checker always rejects, even a "good" partial solution that
    /// produces the correct output will be considered wrong.  This means the
    /// good partial will appear to fail the subtask, and the task should
    /// return `PartialSolutionFailsSubtask`.
    #[test]
    fn always_reject_checker_good_solution_errors() {
        let tempdir = TempDir::new().unwrap();
        let task_path = tempdir.path().join("always_reject_good");

        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 1 << "\n"; return 0; }
        "#;

        let good_partial = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 1 << "\n"; return 0; }
        "#;

        let task = Task::new("always_reject_good", &task_path)
            .with_debug_level(LevelFilter::Off)
            .with_checker(always_reject)
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new("only subtask").with_test(2, || "".to_owned()))
            .with_partial_solution(good_partial, &[0]); // declared to pass subtask 0

        // The checker rejects every answer so the good partial is considered
        // failing – expect PartialSolutionFailsSubtask.
        let result = task.run();
        assert!(
            matches!(result, Err(Error::PartialSolutionFailsSubtask { .. })),
            "Expected PartialSolutionFailsSubtask, got: {result:?}"
        );
    }

    // ─── exact_checker (stricter than default) ───────────────────────────────

    /// The exact checker rejects a solution whose output differs only by
    /// trailing whitespace, whereas the default diff_checker would accept it.
    /// In this test the "bad" partial produces trailing spaces, so with the
    /// exact checker it should be findable as wrong.
    #[test]
    fn exact_checker_rejects_trailing_whitespace() {
        let tempdir = TempDir::new().unwrap();
        let task_path = tempdir.path().join("exact_trailing");

        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << "42\n"; return 0; }
        "#;

        // Bad solution outputs "42   \n" (trailing spaces).
        // The default diff_checker would accept this; exact_checker will not.
        let bad_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << "42   \n"; return 0; }
        "#;

        let task = Task::new("exact_trailing", &task_path)
            .with_debug_level(LevelFilter::Off)
            .with_checker(exact_checker)
            .with_solution_source(main_solution)
            // Use with_test so we have a concrete test case.
            .with_subtask(Subtask::new("only subtask").with_test(2, || "".to_owned()))
            .with_partial_solution(bad_solution, &[]) // expected to fail
            .with_min_failures(1);

        // With exact_checker the bad solution is always wrong → robust tests
        // can be found → task succeeds.
        task.run().unwrap();
    }

    // ─── real use-case: "any valid answer" problem ──────────────────────────

    /// Simulates a problem with multiple correct outputs: given a number n,
    /// print any non-negative integer.  The main solution prints n itself;
    /// a "good" alternate solution prints 0 (also valid).  A "bad" solution
    /// prints -1 (not a non-negative integer), which must be rejected.
    #[test]
    fn any_nonneg_integer_checker_end_to_end() {
        let tempdir = TempDir::new().unwrap();
        let task_path = tempdir.path().join("any_nonneg");

        // Main solution: prints the input number.
        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { int n; cin >> n; cout << n << "\n"; return 0; }
        "#;

        // Good partial: always prints 0 – valid per the checker.
        let good_partial = r#"
        #include <iostream>
        using namespace std;
        int main() { int n; cin >> n; cout << 0 << "\n"; return 0; }
        "#;

        // Bad solution: prints -1 – invalid per the checker.
        let bad_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { int n; cin >> n; cout << -1 << "\n"; return 0; }
        "#;

        let task = Task::new("any_nonneg", &task_path)
            .with_debug_level(LevelFilter::Off)
            .with_checker(any_nonneg_integer_checker)
            .with_solution_source(main_solution)
            .with_subtask(
                Subtask::new("only subtask")
                    // Generate inputs: single integer 1..=100.
                    .with_test(0, || {
                        use rand::Rng;
                        let mut rng = rand::rng();
                        format!("{}", rng.random_range(1_i32..=100))
                    }),
            )
            .with_partial_solution(good_partial, &[0]) // passes subtask 0
            .with_partial_solution(bad_solution, &[]) // fails all subtasks
            .with_min_failures(3);

        task.run().unwrap();
    }
}
