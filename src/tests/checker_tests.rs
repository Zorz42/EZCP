#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod checker_tests {
    use crate::tests::generic_tests::generic_tests::test_task;
    use crate::{Error, Mode, Result, Subtask, Task};

    /// A program that prints `output`, a C++ string literal's contents.
    fn printing(output: &str) -> String {
        format!("#include <iostream>\nint main() {{ std::cout << \"{output}\"; }}")
    }

    /// Runs `main` and the partial solutions on one subtask of two empty tests.
    fn run(checker: Option<fn(&str, &str, &str) -> bool>, main: &str, partials: &[(&str, &[usize])]) -> Result<()> {
        let (_dir, task) = test_task();
        let mut task = task
            .with_solution_source(main)
            .with_subtask(Subtask::new(0, "only subtask").with_test(2, |_rng| String::new()))
            .with_min_failures(1)
            .with_max_tries(10);
        if let Some(checker) = checker {
            task = task.with_checker(checker);
        }
        partials
            .iter()
            .fold(task, |task, (source, passes)| task.with_partial_solution("partial", source, passes))
            .run_mode(Mode::Files)
    }

    #[test]
    fn default_checker_accepts_trailing_whitespace() {
        run(None, &printing("42\\n"), &[(&printing("42\\n\\n"), &[0])]).unwrap();
    }

    #[test]
    fn default_checker_rejects_different_output() {
        run(None, &printing("1\\n"), &[(&printing("2\\n"), &[])]).unwrap();
    }

    #[test]
    fn exact_checker_rejects_trailing_whitespace() {
        run(Some(|_, official, output| official == output), &printing("42\\n"), &[(&printing("42   \\n"), &[])]).unwrap();
    }

    #[test]
    fn always_accept_checker_accepts_any_output() {
        run(Some(|_, _, _| true), &printing("42\\n"), &[(&printing("0\\n"), &[0])]).unwrap();
    }

    /// Nothing can break the bad solution, so the run has to fail.
    #[test]
    fn always_accept_checker_with_bad_solution() {
        let result = run(Some(|_, _, _| true), &printing("1\\n"), &[(&printing("999\\n"), &[])]);
        assert!(
            matches!(
                result,
                Err(Error::PartialSolutionPassesExtraSubtask {
                    subtask_number: 1,
                    partial_number: 1,
                    ..
                })
            ),
            "got {result:?}"
        );
    }

    #[test]
    fn always_reject_checker_errors_on_main_solution() {
        let result = run(Some(|_, _, _| false), &printing("1\\n"), &[]);
        assert!(matches!(result, Err(Error::SolutionFailed { .. })), "got {result:?}");
    }

    #[test]
    fn checker_rejects_good_partial_solution_errors() {
        let result = run(Some(|_, _, output| output.trim() == "main"), &printing("main\\n"), &[(&printing("partial\\n"), &[0])]);
        assert!(matches!(result, Err(Error::PartialSolutionFailsSubtask { .. })), "got {result:?}");
    }

    #[test]
    fn any_nonneg_integer_checker_end_to_end() {
        let echo = |answer: &str| format!("#include <iostream>\nint main() {{ int n; std::cin >> n; std::cout << {answer} << '\\n'; }}");
        let (_dir, task): (_, Task<String>) = test_task();
        task.with_checker(|_, _, output| output.trim().parse::<i64>().is_ok_and(|value| value >= 0))
            .with_solution_source(&echo("n"))
            .with_subtask(Subtask::new(0, "only subtask").with_test(0, |rng| rng.random_range(1_i32..=100).to_string()))
            .with_partial_solution("good", &echo("0"), &[0])
            .with_partial_solution("bad", &echo("-1"), &[])
            .with_min_failures(3)
            .run_mode(Mode::Files)
            .unwrap();
    }
}
