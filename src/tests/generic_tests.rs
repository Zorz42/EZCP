#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod generic_tests {
    use crate::{Error, Subtask, Task};
    use std::path::{Path};
    use tempfile::TempDir;
    use crate::logger::DebugLevel;

    pub struct Test {
        pub task: Task,
        task_path: TempDir,
    }

    impl Test {
        pub fn new() -> Self {
            let task_path = TempDir::new().expect("Failed to create temporary directory");
            let mut task = Task::new("Test task", task_path.path());
            task.debug_level = DebugLevel::Detailed;
            Test { task, task_path }
        }

        pub fn create_solution(&mut self, contents: &str) {
            std::fs::write(self.task_path.path().join("solution.cpp"), contents).unwrap();
        }

        pub fn task_path(&self) -> &Path {
            self.task_path.path()
        }

        pub fn test(mut self) {
            for _ in 0..10 {
                self.task.create_tests().unwrap();
            }
            // Clean up the temporary directory
            drop(self.task_path);
        }
    }

    #[test]
    fn create_empty() {
        let task = Test::new();

        // create solution file
        let solution_contents = "int main() { return 0; }";
        std::fs::write(task.task_path().join("solution.cpp"), solution_contents).unwrap();

        task.test()
    }

    #[test]
    fn create_with_subtasks() {
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

        task.create_solution(solution_contents);

        let subtask1 = Subtask::new();
        let subtask2 = Subtask::new();
        let subtask3 = Subtask::new();

        // create subtasks
        task.task.add_subtask(subtask1);
        task.task.add_subtask(subtask2);
        task.task.add_subtask(subtask3);

        task.test()
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

        task.create_solution(solution_contents);

        let mut subtask1 = Subtask::new();
        subtask1.add_test_str("1\n");
        subtask1.add_test_str("2\n");
        subtask1.add_test_str("3\n");
        let mut subtask2 = Subtask::new();
        subtask2.add_test_str("1\n");
        subtask2.add_test_str("2\n");
        subtask2.add_test_str("3\n");
        let mut subtask3 = Subtask::new();
        subtask3.add_test_str("1\n");
        subtask3.add_test_str("2\n");

        // create subtasks
        task.task.add_subtask(subtask1);
        task.task.add_subtask(subtask2);
        task.task.add_subtask(subtask3);

        task.test()
    }

    #[test]
    fn test_fails_without_solution() {
        let mut task = Test::new();

        for _ in 0..10 {
            assert!(matches!(task.task.create_tests(), Err(Error::MissingSolutionFile { .. })));
            assert!(matches!(task.task.create_tests(), Err(Error::MissingSolutionFile { .. })));
        }
    }

    #[test]
    fn test_times_out() {
        let mut task = Test::new();
        task.task.time_limit = 0.1;

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
            cout<<fib(1000)<<"\n";
            return 0;
        }

        
        "#;

        task.create_solution(solution_contents);

        let mut subtask1 = Subtask::new();
        subtask1.add_test_str("1\n");

        // create subtasks
        task.task.add_subtask(subtask1);

        for _ in 0..3 {
            assert!(matches!(task.task.create_tests(), Err(Error::SolutionTimedOut { .. })));
        }
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

        task.create_solution(solution_contents);

        let mut subtask1 = Subtask::new();
        subtask1.add_test_str("1\n");

        // create subtasks
        task.task.add_subtask(subtask1);

        for _ in 0..10 {
            assert!(matches!(task.task.create_tests(), Err(Error::CompilerError { .. })));
        }
    }

    #[test]
    fn create_with_custom_names() {
        let mut task = Test::new();

        task.task.get_input_file_name = Box::new(|test_id: i32, subtask_id: i32, test_id_in_subtask: i32| format!("in_{subtask_id}_{test_id_in_subtask}_{test_id}.txt"));
        task.task.get_output_file_name = Box::new(|test_id: i32, subtask_id: i32, test_id_in_subtask: i32| format!("out_{subtask_id}_{test_id_in_subtask}_{test_id}.txt"));

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            cout<<"1\n";
            return 0; 
        }
        
        "#;

        task.create_solution(solution_contents);

        let mut subtask1 = Subtask::new();
        subtask1.add_test_str("1\n");
        subtask1.add_test_str("2\n");
        subtask1.add_test_str("3\n");
        let mut subtask2 = Subtask::new();
        subtask2.add_test_str("1\n");
        subtask2.add_test_str("2\n");
        subtask2.add_test_str("3\n");
        let mut subtask3 = Subtask::new();
        subtask3.add_test_str("1\n");
        subtask3.add_test_str("2\n");

        // create subtasks
        task.task.add_subtask(subtask1);
        task.task.add_subtask(subtask2);
        task.task.add_subtask(subtask3);

        task.test()
    }

    #[test]
    fn create_with_custom_names2() {
        let mut task = Test::new();

        task.task.get_input_file_name = Box::new(|_test_id: i32, subtask_id: i32, test_id_in_subtask: i32| format!("in_{subtask_id}_{test_id_in_subtask}.txt"));
        task.task.get_output_file_name = Box::new(|_test_id: i32, subtask_id: i32, test_id_in_subtask: i32| format!("out_{subtask_id}_{test_id_in_subtask}.txt"));

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            cout<<"1\n";
            return 0; 
        }
        
        "#;

        task.create_solution(solution_contents);

        let mut subtask1 = Subtask::new();
        subtask1.add_test_str("1\n");
        subtask1.add_test_str("2\n");
        subtask1.add_test_str("3\n");
        let mut subtask2 = Subtask::new();
        subtask2.add_test_str("1\n");
        subtask2.add_test_str("2\n");
        subtask2.add_test_str("3\n");
        let mut subtask3 = Subtask::new();
        subtask3.add_test_str("1\n");
        subtask3.add_test_str("2\n");

        // create subtasks
        task.task.add_subtask(subtask1);
        task.task.add_subtask(subtask2);
        task.task.add_subtask(subtask3);

        task.test()
    }

    #[test]
    fn create_with_custom_names3() {
        let mut task = Test::new();

        task.task.get_input_file_name = Box::new(|test_id: i32, _subtask_id: i32, _test_id_in_subtask: i32| format!("in_{test_id}.txt"));
        task.task.get_output_file_name = Box::new(|test_id: i32, _subtask_id: i32, _test_id_in_subtask: i32| format!("out_{test_id}.txt"));

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            cout<<"1\n";
            return 0; 
        }
        
        "#;

        task.create_solution(solution_contents);

        let mut subtask1 = Subtask::new();
        subtask1.add_test_str("1\n");
        subtask1.add_test_str("2\n");
        subtask1.add_test_str("3\n");
        let mut subtask2 = Subtask::new();
        subtask2.add_test_str("1\n");
        subtask2.add_test_str("2\n");
        subtask2.add_test_str("3\n");
        let mut subtask3 = Subtask::new();
        subtask3.add_test_str("1\n");
        subtask3.add_test_str("2\n");

        // create subtasks
        task.task.add_subtask(subtask1);
        task.task.add_subtask(subtask2);
        task.task.add_subtask(subtask3);

        task.test()
    }
}
