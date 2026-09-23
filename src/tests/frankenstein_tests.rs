//! Subtasks whose tests have to break several partial solutions with different weak spots.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod frankenstein_tests {
    use crate::Mode;
    use crate::tests::test_shared::initialize_logger;
    use crate::{Error, Subtask, Task};
    use tempfile::TempDir;

    const MAIN: &str = "
        #include <iostream>
        using namespace std;
        int main() {
            int n; cin >> n;
            cout << n << endl;
            return 0;
        }
        ";

    /// `wrong_when` is a C++ condition on `n`.
    fn partial(wrong_when: &str) -> String {
        format!(
            "
        #include <iostream>
        using namespace std;
        int main() {{
             int n; cin >> n;
             if ({wrong_when}) cout << n + 1 << endl;
             else cout << n << endl;
             return 0;
        }}
        "
        )
    }

    fn test_inputs(task_path: &std::path::Path) -> Vec<i32> {
        std::fs::read_dir(task_path.join("tests"))
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                path.extension().is_some_and(|ext| ext == "in").then(|| std::fs::read_to_string(path).unwrap().trim().parse().unwrap())
            })
            .collect()
    }

    /// A test in 11..20 breaks both at once; every kept test must break at least one.
    #[test]
    fn overlapping_weak_spots_are_both_covered() {
        initialize_logger();
        let tempdir = TempDir::new().unwrap();
        let task_name = "frankenstein_overlap";
        let task_path = tempdir.path().join(task_name);

        let task = Task::new(task_name, &task_path)
            .with_solution_source(MAIN)
            .with_subtask(Subtask::new(0, "").with_test(0, |rng| format!("{}", rng.random_range(0..50))))
            .with_partial_solution("bad above ten", &partial("n > 10"), &[])
            .with_partial_solution("bad below twenty", &partial("n < 20"), &[])
            .with_min_failures(5);

        task.run_mode(Mode::Files).unwrap();

        let inputs = test_inputs(&task_path);
        assert!(inputs.iter().all(|&n| n > 10 || n < 20), "a test breaks neither partial solution: {inputs:?}");
        assert!(inputs.iter().filter(|&&n| n > 10).count() >= 5, "too few tests break the first partial solution: {inputs:?}");
        assert!(inputs.iter().filter(|&&n| n < 20).count() >= 5, "too few tests break the second partial solution: {inputs:?}");
    }

    /// No test breaks both, so this only finishes because failures are counted
    /// per solution.
    #[test]
    fn disjoint_weak_spots_still_finish() {
        initialize_logger();
        let tempdir = TempDir::new().unwrap();
        let task_name = "frankenstein_disjoint";
        let task_path = tempdir.path().join(task_name);

        let task = Task::new(task_name, &task_path)
            .with_solution_source(MAIN)
            .with_subtask(Subtask::new(0, "").with_test(0, |rng| format!("{}", rng.random_range(0..50))))
            .with_partial_solution("bad above", &partial("n >= 25"), &[])
            .with_partial_solution("bad below", &partial("n < 25"), &[])
            .with_min_failures(3);

        task.run_mode(Mode::Files).unwrap();

        let inputs = test_inputs(&task_path);
        assert!(inputs.iter().filter(|&&n| n >= 25).count() >= 3, "too few tests break the first partial solution: {inputs:?}");
        assert!(inputs.iter().filter(|&&n| n < 25).count() >= 3, "too few tests break the second partial solution: {inputs:?}");
    }

    /// Reported right after the first subtask, not after all of them.
    #[test]
    fn a_partial_solution_that_never_fails_is_reported_at_its_subtask() {
        initialize_logger();
        let tempdir = TempDir::new().unwrap();
        let task_name = "frankenstein_never_fails";
        let task_path = tempdir.path().join(task_name);

        let task = Task::new(task_name, &task_path)
            .with_solution_source(MAIN)
            .with_subtask(Subtask::new(0, "first").with_test(3, |rng| format!("{}", rng.random_range(0..50))))
            .with_subtask(Subtask::new(0, "second").with_test(3, |rng| format!("{}", rng.random_range(0..50))))
            .with_partial_solution("secretly correct", &partial("false"), &[])
            .with_min_failures(1)
            .with_max_tries(3);

        let err = task.run_mode(Mode::Files).unwrap_err();
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
