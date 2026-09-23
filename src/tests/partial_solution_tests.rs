#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod partial_solution_tests {
    use crate::tests::generic_tests::generic_tests::test_task;
    use crate::{Error, Mode, Subtask, array_generator};

    const SUM: &str = "#include <iostream>
        int main() { int n; std::cin >> n; long long sum = 0; for (int i = 0; i < n; i++) { int a; std::cin >> a; sum += a; } std::cout << sum << '\\n'; }";

    /// Runs `partial` against [`SUM`] on a subtask of small values and one whose
    /// sums overflow `int`.
    fn run_against_sum(partial: &str, passes_subtasks: &[usize]) {
        let (_dir, task) = test_task();
        task.with_solution_source(SUM)
            .with_subtask(Subtask::new(0, "").with_test(5, array_generator(1, 100, 1, 100)))
            .with_subtask(Subtask::new(0, "").with_test(5, array_generator(1, 100, 1, 1_000_000_000)))
            .with_partial_solution("partial", partial, passes_subtasks)
            .run_mode(Mode::Files)
            .unwrap();
    }

    #[test]
    fn test_partial_solution_that_overflows() {
        run_against_sum(
            "#include <iostream>
            int main() { int n; std::cin >> n; int sum = 0; for (int i = 0; i < n; i++) { int a; std::cin >> a; sum += a; } std::cout << sum << '\\n'; }",
            &[0],
        );
    }

    #[test]
    fn test_partial_solution_tle() {
        run_against_sum(
            "#include <iostream>
            int main() { int n; std::cin >> n; int sum = 0; for (int i = 0; i < n; i++) { int a; std::cin >> a; while (a--) sum++; } std::cout << sum << '\\n'; }",
            &[0],
        );
    }

    #[test]
    fn test_partial_solution_crash() {
        run_against_sum("int main() { int* n = nullptr; while (true) { *n = 1; n++; } }", &[]);
    }

    #[test]
    fn test_partial_solution_tle_everywhere() {
        run_against_sum(
            "#include <iostream>
            int main() { int n; std::cin >> n; int sum = 0; for (int i = 0; i < n; i++) { long long a; std::cin >> a; while (a++) sum++; } std::cout << sum << '\\n'; }",
            &[],
        );
    }

    #[test]
    fn test_partial_solution_that_is_never_broken_is_reported() {
        let echo = "#include <iostream>\nint main() { int n; std::cin >> n; std::cout << n << '\\n'; }";
        let (_dir, task) = test_task();
        let task = task
            .with_solution_source(echo)
            .with_subtask(Subtask::new(0, "first").with_test(2, |_rng| "1\n".to_owned()))
            .with_subtask(Subtask::new(0, "second").with_test(2, |_rng| "2\n".to_owned()))
            .with_partial_solution("indistinguishable", echo, &[0])
            .with_min_failures(1)
            .with_max_tries(3);

        assert!(matches!(
            task.run_mode(Mode::Files),
            Err(Error::PartialSolutionPassesExtraSubtask {
                subtask_number: 2,
                partial_number: 1,
                ..
            })
        ));
    }

    #[test]
    fn test_partial_solution_with_unknown_subtask_index_is_rejected() {
        let solution = "int main() { return 0; }";
        let (_dir, task) = test_task();
        let task = task
            .with_solution_source(solution)
            .with_subtask(Subtask::new(0, "only subtask").with_test(1, |_rng| "1\n".to_owned()))
            .with_partial_solution("partial", solution, &[1]);

        assert!(matches!(
            task.run_mode(Mode::Files),
            Err(Error::InvalidSubtaskIndex {
                subtask_number: 1,
                num_subtasks: 1,
                partial_number: 1,
                ..
            })
        ));
    }
}
