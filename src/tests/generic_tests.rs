#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod generic_tests {
    use crate::to_output::ToOutput;
    use crate::{Error, Subtask, Task};
    use log::LevelFilter;
    use tempfile::TempDir;

    pub struct Test<T: ToOutput> {
        pub task: Task<T>,
        task_path: TempDir,
    }

    impl<T: ToOutput> Test<T> {
        pub fn new() -> Self {
            let task_path = TempDir::new().unwrap();
            let task = Task::new("Test task", task_path.path()).with_debug_level(LevelFilter::Trace);
            Self { task, task_path }
        }

        pub fn test(self) {
            self.task.run().unwrap();
            // Clean up the temporary directory
            drop(self.task_path);
        }
    }

    #[test]
    fn create_empty() {
        let mut task = Test::<String>::new();

        // create solution file
        let solution_contents = "int main() { return 0; }";
        task.task = task.task.with_solution_source(solution_contents);

        task.test();
    }

    #[test]
    fn create_with_subtasks() {
        let mut task = Test::<String>::new();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            cout<<"1\n";
            return 0; 
        }
        
        "#;

        task.task = task.task.with_solution_source(solution_contents);

        let subtask1 = Subtask::new(0, "");
        let subtask2 = Subtask::new(0, "");
        let subtask3 = Subtask::new(0, "");

        // create subtasks
        task.task = task.task.with_subtask(subtask1).with_subtask(subtask2).with_subtask(subtask3);

        task.test();
    }

    #[test]
    fn create_with_tests() {
        let mut task = Test::new();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            cout<<"1\n";
            return 0; 
        }
        
        "#;

        task.task = task.task.with_solution_source(solution_contents);

        let subtask1 = Subtask::new(0, "")
            .with_test(1, || "1\n".to_owned())
            .with_test(1, || "2\n".to_owned())
            .with_test(1, || "3\n".to_owned());
        let subtask2 = Subtask::new(0, "")
            .with_test(1, || "1\n".to_owned())
            .with_test(1, || "2\n".to_owned())
            .with_test(1, || "3\n".to_owned());
        let subtask3 = Subtask::new(0, "").with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned());

        // create subtasks
        task.task = task.task.with_subtask(subtask1).with_subtask(subtask2).with_subtask(subtask3);

        task.test();
    }

    #[test]
    fn test_fails_without_solution() {
        let task = Test::<String>::new();

        assert!(matches!(task.task.run(), Err(Error::MissingSolution)));
    }

    #[test]
    fn test_times_out() {
        let mut task = Test::new();
        task.task = task.task.with_time_limit(100);

        // create solution file
        let solution_contents = r#"
        #include<iostream>
        using namespace std;

        int fib(int a){
                if(a<=2)
                        return 1;
                return fib(a-1)+fib(a-2);
        }

        int main() {
            cout<<fib(100)<<"\n";
            return 0;
        }
        "#;

        task.task = task.task.with_solution_source(solution_contents);

        let subtask1 = Subtask::new(0, "").with_test(1, || "1\n".to_owned());

        // create subtasks
        task.task = task.task.with_subtask(subtask1);

        assert!(matches!(task.task.run(), Err(Error::SolutionTimedOut { .. })));
    }

    #[test]
    fn test_compile_error() {
        let mut task = Test::new();

        // create solution file
        let solution_contents = "
        int main() {
            this is a compile error
            return 0;
        }
        ";

        task.task = task.task.with_solution_source(solution_contents);

        let subtask1 = Subtask::new(0, "").with_test(1, || "1\n".to_owned());

        // create subtasks
        task.task = task.task.with_subtask(subtask1);

        assert!(matches!(task.task.run(), Err(Error::CompilerError { .. })));
    }

    #[test]
    fn create_with_custom_names() {
        let mut task = Test::new();

        task.task = task
            .task
            .with_get_input_file_name(|test_id: i32, subtask_id: i32, test_id_in_subtask: i32| format!("in_{subtask_id}_{test_id_in_subtask}_{test_id}.txt"))
            .with_get_output_file_name(|test_id: i32, subtask_id: i32, test_id_in_subtask: i32| format!("out_{subtask_id}_{test_id_in_subtask}_{test_id}.txt"));

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            cout<<"1\n";
            return 0; 
        }
        "#;

        task.task = task.task.with_solution_source(solution_contents);

        let subtask1 = Subtask::new(0, "")
            .with_test(1, || "1\n".to_owned())
            .with_test(1, || "2\n".to_owned())
            .with_test(1, || "3\n".to_owned());
        let subtask2 = Subtask::new(0, "")
            .with_test(1, || "1\n".to_owned())
            .with_test(1, || "2\n".to_owned())
            .with_test(1, || "3\n".to_owned());
        let subtask3 = Subtask::new(0, "").with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned());

        // create subtasks
        task.task = task.task.with_subtask(subtask1).with_subtask(subtask2).with_subtask(subtask3);

        task.test();
    }

    #[test]
    fn create_with_custom_names2() {
        let mut task = Test::new();

        task.task = task
            .task
            .with_get_input_file_name(|_test_id: i32, subtask_id: i32, test_id_in_subtask: i32| format!("in_{subtask_id}_{test_id_in_subtask}.txt"))
            .with_get_output_file_name(|_test_id: i32, subtask_id: i32, test_id_in_subtask: i32| format!("out_{subtask_id}_{test_id_in_subtask}.txt"));

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            cout<<"1\n";
            return 0; 
        }
        
        "#;

        task.task = task.task.with_solution_source(solution_contents);

        let subtask1 = Subtask::new(0, "")
            .with_test(1, || "1\n".to_owned())
            .with_test(1, || "2\n".to_owned())
            .with_test(1, || "3\n".to_owned());
        let subtask2 = Subtask::new(0, "")
            .with_test(1, || "1\n".to_owned())
            .with_test(1, || "2\n".to_owned())
            .with_test(1, || "3\n".to_owned());
        let subtask3 = Subtask::new(0, "").with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned());

        // create subtasks
        task.task = task.task.with_subtask(subtask1).with_subtask(subtask2).with_subtask(subtask3);

        task.test();
    }

    #[test]
    fn create_with_custom_names3() {
        let mut task = Test::new();

        task.task = task
            .task
            .with_get_input_file_name(|test_id: i32, _subtask_id: i32, _test_id_in_subtask: i32| format!("in_{test_id}.txt"))
            .with_get_output_file_name(|test_id: i32, _subtask_id: i32, _test_id_in_subtask: i32| format!("out_{test_id}.txt"));

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            cout<<"1\n";
            return 0; 
        }
        
        "#;

        task.task = task.task.with_solution_source(solution_contents);

        let subtask1 = Subtask::new(0, "")
            .with_test(1, || "1\n".to_owned())
            .with_test(1, || "2\n".to_owned())
            .with_test(1, || "3\n".to_owned());
        let subtask2 = Subtask::new(0, "")
            .with_test(1, || "1\n".to_owned())
            .with_test(1, || "2\n".to_owned())
            .with_test(1, || "3\n".to_owned());
        let subtask3 = Subtask::new(0, "").with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned());

        // create subtasks
        task.task = task.task.with_subtask(subtask1).with_subtask(subtask2).with_subtask(subtask3);

        task.test();
    }

    // --- Task-level edge cases from ANALYSIS.md ---

    #[test]
    fn test_task_no_subtasks_succeeds() {
        // A task with a solution but no subtasks should complete successfully
        // (the implementation warns but does not return an error).
        let mut task = Test::<String>::new();
        task.task = task.task.with_solution_source("int main() { return 0; }");
        task.task.run().unwrap();
    }

    /// Every edit to a solution compiles to a binary of its own, so a build
    /// folder that is never swept keeps growing with binaries no run will ever
    /// use again.
    #[test]
    fn test_stale_build_artifacts_are_removed_on_the_next_run() {
        let tempdir = TempDir::new().unwrap();
        let task_path = tempdir.path().join("stale_artifacts");

        let sources_in_build_folder = || {
            let mut sources = std::fs::read_dir(task_path.join("build"))
                .unwrap()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "cpp"))
                .collect::<Vec<_>>();
            sources.sort();
            sources
        };

        let run_with = |solution: &str| {
            Task::new("stale artifacts", &task_path)
                .with_solution_source(solution)
                .with_subtask(Subtask::new(0, "").with_test(1, || "1\n".to_owned()))
                .run()
                .unwrap();
        };

        run_with("int main() { return 0; }");
        // The timer and the solution.
        let after_first_run = sources_in_build_folder();
        assert_eq!(after_first_run.len(), 2, "expected the timer and one solution, got {after_first_run:?}");

        run_with("int main() { return 1 - 1; }");
        let after_second_run = sources_in_build_folder();
        assert_eq!(after_second_run.len(), 2, "the first solution should not have been kept, got {after_second_run:?}");
        assert_ne!(after_first_run, after_second_run, "the edited solution should have replaced the original one");
    }

    /// A naming closure that ignores the ids it is given maps every test to the
    /// same file. Without a check the tests would overwrite each other and the
    /// archive would end up with one entry where the task promised many.
    #[test]
    fn test_colliding_test_file_names_are_reported() {
        let mut task = Test::new();

        task.task = task
            .task
            .with_solution_source("int main() { return 0; }")
            .with_get_input_file_name(|_test_id, _subtask_id, _test_id_in_subtask| "test.in".to_owned())
            .with_get_output_file_name(|_test_id, _subtask_id, _test_id_in_subtask| "test.out".to_owned())
            .with_subtask(Subtask::new(0, "").with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned()));

        assert!(matches!(task.task.run(), Err(Error::TestAlreadyExists { .. })));
    }

    #[test]
    fn test_task_large_time_limit_does_not_panic() {
        let mut task = Test::new();
        task.task = task.task.with_time_limit(1_000_000).with_solution_source("int main() { return 0; }");
        let subtask = Subtask::new(0, "").with_test(1, || "\n".to_owned());
        task.task = task.task.with_subtask(subtask);
        // Should complete without panicking; may succeed or error but must not panic
        let _ = task.task.run();
    }
}
