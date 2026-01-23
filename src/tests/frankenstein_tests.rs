#[cfg(test)]
mod frankenstein_tests {
    use crate::tests::test_shared::initialize_logger;
    use crate::{Subtask, Task};
    use rand::Rng;
    use tempfile::TempDir;

    #[test]
    fn test_frankenstein_overlap() {
        initialize_logger();
        let tempdir = TempDir::new().unwrap();
        let task_name = "frankenstein_overlap";
        let task_path = tempdir.path().join(task_name);

        // Main Correct Solution: prints input
        let source_main = "
        #include <iostream>
        using namespace std;
        int main() {
            int n; cin >> n;
            cout << n << endl;
            return 0;
        }
        ";

        // Bad Solution A: Fails if x > 10 (prints x+1)
        let source_bad_a = "
        #include <iostream>
        using namespace std;
        int main() {
             int n; cin >> n;
             if (n > 10) cout << n + 1 << endl;
             else cout << n << endl;
             return 0;
        }
        ";

        // Bad Solution B: Fails if x < 20 (prints x-1)
        // Overlap where BOTH fail: 10 < x < 20.
        // E.g. x=15. A prints 16. B prints 14. Main prints 15.
        // Both A and B are wrong.
        let source_bad_b = "
        #include <iostream>
        using namespace std;
        int main() {
             int n; cin >> n;
             if (n < 20) cout << n - 1 << endl;
             else cout << n << endl;
             return 0;
        }
        ";

        let subtask = Subtask::new().with_test(0, || {
            let mut rng = rand::rng();
            format!("{}", rng.random_range(0..50))
        });

        let task = Task::new(task_name, &task_path)
            .with_solution_source(source_main)
            .with_subtask(subtask)
            // Solution A should fail subtask 0
            .with_partial_solution(source_bad_a, &[])
            // Solution B should fail subtask 0
            .with_partial_solution(source_bad_b, &[])
            .with_min_failures(5);

        // Run task
        // It should succeed, finding tests in range (10, 20).

        let res = task.run();
        res.unwrap();

        // Verify generated tests are in range (10, 20)
        // We can read the generated tests from task.tests_path
        let tests_dir = task_path.join("tests");
        let entries = std::fs::read_dir(tests_dir).unwrap();
        let mut count = 0;
        for entry in entries {
            let path = entry.unwrap().path();
            if path.extension().unwrap() == "in" {
                let content = std::fs::read_to_string(path).unwrap();
                let n: i32 = content.trim().parse().unwrap();
                assert!(n > 10 && n < 20, "Generated test {n} is not in overlap range (10, 20)");
                count += 1;
            }
        }
        assert!(count >= 5, "Should have generated at least 5 tests");

        drop(tempdir);
    }
}
