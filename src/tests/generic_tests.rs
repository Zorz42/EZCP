#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod generic_tests {
    use crate::{Error, Subtask, Task};
    use log::LevelFilter;
    use tempfile::TempDir;

    pub struct Test {
        pub task: Task,
        task_path: TempDir,
    }

    impl Test {
        pub fn new() -> Self {
            let task_path = TempDir::new().unwrap();
            let mut task = Task::new("Test task", task_path.path());
            task.debug_level = LevelFilter::Trace;
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
        let mut task = Test::new();

        // create solution file
        let solution_contents = "int main() { return 0; }";
        task.task = task.task.with_solution_source(solution_contents.to_owned());

        task.test();
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

        task.task = task.task.with_solution_source(solution_contents.to_owned());

        let subtask1 = Subtask::new();
        let subtask2 = Subtask::new();
        let subtask3 = Subtask::new();

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

        task.task = task.task.with_solution_source(solution_contents.to_owned());

        let subtask1 = Subtask::new().with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned()).with_test(1, || "3\n".to_owned());
        let subtask2 = Subtask::new().with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned()).with_test(1, || "3\n".to_owned());
        let subtask3 = Subtask::new().with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned());

        // create subtasks
        task.task = task.task.with_subtask(subtask1).with_subtask(subtask2).with_subtask(subtask3);

        task.test();
    }

    #[test]
    fn test_fails_without_solution() {
        let task = Test::new();

        assert!(matches!(task.task.run(), Err(Error::MissingSolution { .. })));
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
            cout<<fib(100)<<"\n";
            return 0;
        }
        "#;

        task.task = task.task.with_solution_source(solution_contents.to_owned());

        let subtask1 = Subtask::new().with_test(1, || "1\n".to_owned());

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

        task.task = task.task.with_solution_source(solution_contents.to_owned());

        let subtask1 = Subtask::new().with_test(1, || "1\n".to_owned());

        // create subtasks
        task.task = task.task.with_subtask(subtask1);

        assert!(matches!(task.task.run(), Err(Error::CompilerError { .. })));
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

        task.task = task.task.with_solution_source(solution_contents.to_owned());

        let subtask1 = Subtask::new().with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned()).with_test(1, || "3\n".to_owned());
        let subtask2 = Subtask::new().with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned()).with_test(1, || "3\n".to_owned());
        let subtask3 = Subtask::new().with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned());

        // create subtasks
        task.task = task.task.with_subtask(subtask1).with_subtask(subtask2).with_subtask(subtask3);

        task.test();
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

        task.task = task.task.with_solution_source(solution_contents.to_owned());

        let subtask1 = Subtask::new().with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned()).with_test(1, || "3\n".to_owned());
        let subtask2 = Subtask::new().with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned()).with_test(1, || "3\n".to_owned());
        let subtask3 = Subtask::new().with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned());

        // create subtasks
        task.task = task.task.with_subtask(subtask1).with_subtask(subtask2).with_subtask(subtask3);

        task.test();
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

        task.task = task.task.with_solution_source(solution_contents.to_owned());

        let subtask1 = Subtask::new().with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned()).with_test(1, || "3\n".to_owned());
        let subtask2 = Subtask::new().with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned()).with_test(1, || "3\n".to_owned());
        let subtask3 = Subtask::new().with_test(1, || "1\n".to_owned()).with_test(1, || "2\n".to_owned());

        // create subtasks
        task.task = task.task.with_subtask(subtask1).with_subtask(subtask2).with_subtask(subtask3);

        task.test();
    }
}
