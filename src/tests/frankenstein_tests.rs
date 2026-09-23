//! Subtasks whose tests have to break several partial solutions with different weak spots.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod frankenstein_tests {
    use crate::tests::generic_tests::generic_tests::test_task;
    use crate::{Error, Mode, Subtask};

    const ECHO: &str = "#include <iostream>\nint main() { int n; std::cin >> n; std::cout << n << std::endl; }";

    /// `wrong_when` is a C++ condition on `n`.
    fn partial(wrong_when: &str) -> String {
        format!("#include <iostream>\nint main() {{ int n; std::cin >> n; std::cout << (({wrong_when}) ? n + 1 : n) << std::endl; }}")
    }

    /// Generates tests from `0..50` that break two partial solutions, wrong where
    /// `wrong_when` holds, `min_failures` times each, and returns the test inputs.
    fn generated_inputs(wrong_when: [&str; 2], min_failures: usize) -> Vec<i32> {
        let (dir, task) = test_task();
        task.with_solution_source(ECHO)
            .with_subtask(Subtask::new(0, "").with_test(0, |rng| rng.random_range(0..50).to_string()))
            .with_partial_solution("first", &partial(wrong_when[0]), &[])
            .with_partial_solution("second", &partial(wrong_when[1]), &[])
            .with_min_failures(min_failures)
            .run_mode(Mode::Files)
            .unwrap();

        std::fs::read_dir(dir.path().join("tests"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "in"))
            .map(|path| std::fs::read_to_string(path).unwrap().trim().parse().unwrap())
            .collect()
    }

    /// A test in 11..20 breaks both at once; every kept test must break at least one.
    #[test]
    fn overlapping_weak_spots_are_both_covered() {
        let inputs = generated_inputs(["n > 10", "n < 20"], 5);
        assert!(inputs.iter().all(|&n| n > 10 || n < 20), "a test breaks neither partial solution: {inputs:?}");
        assert!(inputs.iter().filter(|&&n| n > 10).count() >= 5, "too few tests break the first partial solution: {inputs:?}");
        assert!(inputs.iter().filter(|&&n| n < 20).count() >= 5, "too few tests break the second partial solution: {inputs:?}");
    }

    /// No test breaks both, so this only finishes because failures are counted
    /// per solution.
    #[test]
    fn disjoint_weak_spots_still_finish() {
        let inputs = generated_inputs(["n >= 25", "n < 25"], 3);
        assert!(inputs.iter().filter(|&&n| n >= 25).count() >= 3, "too few tests break the first partial solution: {inputs:?}");
        assert!(inputs.iter().filter(|&&n| n < 25).count() >= 3, "too few tests break the second partial solution: {inputs:?}");
    }

    /// Reported right after the first subtask, not after all of them.
    #[test]
    fn a_partial_solution_that_never_fails_is_reported_at_its_subtask() {
        let (_dir, task) = test_task();
        let err = task
            .with_solution_source(ECHO)
            .with_subtask(Subtask::new(0, "first").with_test(3, |rng| rng.random_range(0..50).to_string()))
            .with_subtask(Subtask::new(0, "second").with_test(3, |rng| rng.random_range(0..50).to_string()))
            .with_partial_solution("secretly correct", &partial("false"), &[])
            .with_min_failures(1)
            .with_max_tries(3)
            .run_mode(Mode::Files)
            .unwrap_err();
        assert!(
            matches!(
                err,
                Error::PartialSolutionPassesExtraSubtask {
                    subtask_number: 1,
                    partial_number: 1,
                    ..
                }
            ),
            "expected the first subtask to report it, got {err}"
        );
    }
}
