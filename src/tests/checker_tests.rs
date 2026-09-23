#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod checker_tests {
    use crate::Mode;
    use crate::tests::generic_tests::generic_tests::Test;
    use crate::{Error, Subtask, Task};
    use log::LevelFilter;
    use tempfile::TempDir;

    fn always_accept(_input: &str, _correct: &str, _program: &str) -> bool {
        true
    }

    fn always_reject(_input: &str, _correct: &str, _program: &str) -> bool {
        false
    }

    fn exact_checker(_input: &str, correct: &str, program: &str) -> bool {
        correct == program
    }

    fn any_nonneg_integer_checker(_input: &str, _correct: &str, program: &str) -> bool {
        program.trim().parse::<i64>().is_ok_and(|v| v >= 0)
    }

    #[test]
    fn default_checker_accepts_trailing_whitespace() {
        let mut task = Test::new();

        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 42 << "\n"; return 0; }
        "#;

        let good_partial = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 42 << "\n\n"; return 0; }
        "#;

        task.task = task
            .task
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new(0, "only subtask").with_test(3, |_rng| String::new()))
            .with_partial_solution("good", good_partial, &[0]);

        task.test(); // must not error
    }

    #[test]
    fn default_checker_rejects_different_output() {
        let mut task = Test::new();

        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 1 << "\n"; return 0; }
        "#;

        let bad_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 2 << "\n"; return 0; }
        "#;

        task.task = task
            .task
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new(0, "only subtask").with_test(3, |_rng| String::new()))
            .with_partial_solution("bad", bad_solution, &[]); // fails all subtasks

        task.test(); // must succeed (bad solution is rejected as expected)
    }

    #[test]
    fn with_checker_returns_self_for_chaining() {
        let tempdir = TempDir::new().unwrap();
        let _task = Task::<String>::new("chain test", tempdir.path())
            .with_debug_level(LevelFilter::Off)
            .with_checker(always_accept)
            .with_solution_source("int main() { return 0; }");
    }

    /// Nothing can break the bad solution, so the run has to fail.
    #[test]
    fn always_accept_checker_with_bad_solution() {
        let tempdir = TempDir::new().unwrap();
        let task_path = tempdir.path().join("always_accept_bad");

        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 1 << "\n"; return 0; }
        "#;

        let bad_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 999 << "\n"; return 0; }
        "#;

        let task = Task::new("always_accept_bad", &task_path)
            .with_debug_level(LevelFilter::Off)
            .with_checker(always_accept)
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new(0, "only subtask").with_test(2, |_rng| String::new()))
            .with_partial_solution("bad", bad_solution, &[])
            .with_min_failures(1)
            .with_max_tries(10);

        assert!(matches!(
            task.run_mode(Mode::Files),
            Err(Error::PartialSolutionPassesExtraSubtask {
                subtask_number: 1,
                partial_number: 1,
                ..
            })
        ));
    }

    #[test]
    fn always_accept_checker_good_solution_any_output() {
        let tempdir = TempDir::new().unwrap();
        let task_path = tempdir.path().join("always_accept_good");

        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 42 << "\n"; return 0; }
        "#;

        let good_partial = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 0 << "\n"; return 0; }
        "#;

        let task = Task::new("always_accept_good", &task_path)
            .with_debug_level(LevelFilter::Off)
            .with_checker(always_accept)
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new(0, "only subtask").with_test(2, |_rng| String::new()))
            .with_partial_solution("good", good_partial, &[0]); // declared to pass subtask 0

        task.run_mode(Mode::Files).unwrap();
    }

    #[test]
    fn always_reject_checker_errors_on_main_solution() {
        let tempdir = TempDir::new().unwrap();
        let task_path = tempdir.path().join("always_reject_main");

        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << 1 << "\n"; return 0; }
        "#;

        let task = Task::<String>::new("always_reject_main", &task_path)
            .with_debug_level(LevelFilter::Off)
            .with_checker(always_reject)
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new(0, "only subtask").with_test(2, |_rng| String::new()));

        let result = task.run_mode(Mode::Files);
        assert!(matches!(result, Err(Error::SolutionFailed { .. })), "Expected SolutionFailed, got: {result:?}");
    }

    #[test]
    fn checker_rejects_good_partial_solution_errors() {
        fn main_only_checker(_input: &str, _correct: &str, program: &str) -> bool {
            program.trim() == "main"
        }

        let tempdir = TempDir::new().unwrap();
        let task_path = tempdir.path().join("reject_good_partial");

        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << "main\n"; return 0; }
        "#;

        let good_partial = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << "partial\n"; return 0; }
        "#;

        let task = Task::<String>::new("reject_good_partial", &task_path)
            .with_debug_level(LevelFilter::Off)
            .with_checker(main_only_checker)
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new(0, "only subtask").with_test(2, |_rng| String::new()))
            .with_partial_solution("good", good_partial, &[0]); // declared to pass subtask 0

        let result = task.run_mode(Mode::Files);
        assert!(
            matches!(result, Err(Error::PartialSolutionFailsSubtask { .. })),
            "Expected PartialSolutionFailsSubtask, got: {result:?}"
        );
    }

    #[test]
    fn exact_checker_rejects_trailing_whitespace() {
        let tempdir = TempDir::new().unwrap();
        let task_path = tempdir.path().join("exact_trailing");

        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << "42\n"; return 0; }
        "#;

        let bad_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { cout << "42   \n"; return 0; }
        "#;

        let task = Task::new("exact_trailing", &task_path)
            .with_debug_level(LevelFilter::Off)
            .with_checker(exact_checker)
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new(0, "only subtask").with_test(2, |_rng| String::new()))
            .with_partial_solution("bad", bad_solution, &[]) // expected to fail
            .with_min_failures(1);

        task.run_mode(Mode::Files).unwrap();
    }

    #[test]
    fn any_nonneg_integer_checker_end_to_end() {
        let tempdir = TempDir::new().unwrap();
        let task_path = tempdir.path().join("any_nonneg");

        let main_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { int n; cin >> n; cout << n << "\n"; return 0; }
        "#;

        let good_partial = r#"
        #include <iostream>
        using namespace std;
        int main() { int n; cin >> n; cout << 0 << "\n"; return 0; }
        "#;

        let bad_solution = r#"
        #include <iostream>
        using namespace std;
        int main() { int n; cin >> n; cout << -1 << "\n"; return 0; }
        "#;

        let task = Task::new("any_nonneg", &task_path)
            .with_debug_level(LevelFilter::Off)
            .with_checker(any_nonneg_integer_checker)
            .with_solution_source(main_solution)
            .with_subtask(Subtask::new(0, "only subtask").with_test(0, |rng| format!("{}", rng.random_range(1_i32..=100))))
            .with_partial_solution("good", good_partial, &[0]) // passes subtask 0
            .with_partial_solution("bad", bad_solution, &[]) // fails all subtasks
            .with_min_failures(3);

        task.run_mode(Mode::Files).unwrap();
    }
}
