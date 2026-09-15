//! Tests for test data stitched together out of counterexamples: a subtask whose
//! tests come from several partial solutions, each broken by a different one.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod frankenstein_tests {
    use crate::Mode;
    use crate::tests::test_shared::initialize_logger;
    use crate::{Error, Subtask, Task};
    use tempfile::TempDir;

    /// Echoes the number it is given.
    const MAIN: &str = "
        #include <iostream>
        using namespace std;
        int main() {
            int n; cin >> n;
            cout << n << endl;
            return 0;
        }
        ";

    /// Builds a solution that answers wrongly exactly where `wrong_when` says so.
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

    /// Every test input a run wrote out.
    fn test_inputs(task_path: &std::path::Path) -> Vec<i32> {
        std::fs::read_dir(task_path.join("tests"))
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                path.extension().is_some_and(|ext| ext == "in").then(|| std::fs::read_to_string(path).unwrap().trim().parse().unwrap())
            })
            .collect()
    }

    /// Two partial solutions with overlapping weak spots: `n > 10` breaks one and
    /// `n < 20` breaks the other, so a test in between breaks both at once.
    ///
    /// Every test that is kept has to break at least one of them - a test that
    /// separates nothing is not worth carrying - and each of them has to be
    /// broken by at least `min_failures` of the tests.
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

    /// Two partial solutions whose weak spots do not overlap at all: `n >= 25`
    /// breaks one and `n < 25` breaks the other, so no single test can ever break
    /// both.
    ///
    /// Counting failures per solution rather than looking for one test that
    /// breaks every solution at once is what makes this finish: requiring the
    /// latter would spend every one of the `max_tries` attempts and still end up
    /// with nothing.
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

    /// A partial solution that is declared to fail a subtask but never does is
    /// reported as soon as that subtask has been generated, rather than after
    /// every remaining subtask has been generated and judged.
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
            // Right everywhere, so no test of either subtask can break it.
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
