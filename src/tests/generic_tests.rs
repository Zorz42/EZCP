#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod generic_tests {
    use crate::{Subtask, Task};
    use std::path::PathBuf;
    use std::sync::Once;

    pub const TESTS_DIR: &str = "test_tasks";
    static INIT: Once = Once::new();

    pub fn initialize_test() {
        INIT.call_once(|| {
            if PathBuf::from(TESTS_DIR).exists() {
                std::fs::remove_dir_all(TESTS_DIR).unwrap();
            }
            std::fs::create_dir_all(TESTS_DIR).unwrap();
        });
    }

    #[test]
    fn create_empty() {
        initialize_test();

        let task_name = "empty";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

        // create solution file
        let solution_contents = "int main() { return 0; }";
        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        for _ in 0..10 {
            assert!(task.create_tests());
        }
    }

    #[test]
    fn create_with_subtasks() {
        initialize_test();

        let task_name = "subtasks";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            cout<<"1\n";
            return 0; 
        }
        
        "#;

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        let subtask1 = Subtask::new(20);
        let subtask2 = Subtask::new(30);
        let subtask3 = Subtask::new(50);

        // create subtasks
        task.add_subtask(subtask1);
        task.add_subtask(subtask2);
        task.add_subtask(subtask3);

        for _ in 0..10 {
            assert!(task.create_tests());
        }
    }

    #[test]
    fn create_with_tests() {
        initialize_test();

        let task_name = "tests";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            cout<<"1\n";
            return 0; 
        }
        
        "#;

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        let mut subtask1 = Subtask::new(20);
        subtask1.add_test_str("1\n");
        subtask1.add_test_str("2\n");
        subtask1.add_test_str("3\n");
        let mut subtask2 = Subtask::new(30);
        subtask2.add_test_str("1\n");
        subtask2.add_test_str("2\n");
        subtask2.add_test_str("3\n");
        let mut subtask3 = Subtask::new(50);
        subtask3.add_test_str("1\n");
        subtask3.add_test_str("2\n");

        // create subtasks
        task.add_subtask(subtask1);
        task.add_subtask(subtask2);
        task.add_subtask(subtask3);

        for _ in 0..10 {
            assert!(task.create_tests());
        }
    }

    #[test]
    fn test_create_multiple_times() {
        initialize_test();

        let task_name = "multiple_times";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            cout<<"1\n";
            return 0; 
        }
        
        "#;

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        let mut subtask1 = Subtask::new(20);
        subtask1.add_test_str("1\n");
        subtask1.add_test_str("2\n");
        subtask1.add_test_str("3\n");
        let mut subtask2 = Subtask::new(30);
        subtask2.add_test_str("4\n");
        subtask2.add_test_str("5\n");
        subtask2.add_test_str("6\n");
        let mut subtask3 = Subtask::new(50);
        subtask3.add_test_str("7\n");
        subtask3.add_test_str("8\n");
        subtask3.add_test_str("9\n");

        // create subtasks
        let subtask1 = task.add_subtask(subtask1);
        let subtask2 = task.add_subtask(subtask2);
        let subtask3 = task.add_subtask(subtask3);

        task.add_subtask_dependency(subtask3, subtask1);
        task.add_subtask_dependency(subtask3, subtask2);

        for _ in 0..100 {
            assert!(task.create_tests());
        }
    }

    #[test]
    fn test_fails_without_solution() {
        initialize_test();

        let task_name = "no_solution";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path).unwrap();

        for _ in 0..10 {
            assert!(!task.create_tests());
        }
    }

    #[test]
    fn test_times_out() {
        initialize_test();

        let task_name = "times_out";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

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

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        let mut subtask1 = Subtask::new(20);
        subtask1.add_test_str("1\n");

        // create subtasks
        task.add_subtask(subtask1);

        for _ in 0..3 {
            assert!(!task.create_tests());
        }
    }

    #[test]
    fn test_compile_error() {
        initialize_test();

        let task_name = "compile_error";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

        // create solution file
        let solution_contents = "
        int main() {
            this is a compile error muahahahahah
            return 0;
        }
        ";

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        let mut subtask1 = Subtask::new(20);
        subtask1.add_test_str("1\n");

        // create subtasks
        task.add_subtask(subtask1);

        for _ in 0..10 {
            assert!(!task.create_tests());
        }
    }
}
