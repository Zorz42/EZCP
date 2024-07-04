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
            assert!(task.create_tests_for_cps());
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
            assert!(task.create_tests_for_cps());
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
            assert!(task.create_tests_for_cps());
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
            assert!(!task.create_tests_for_cps());
        }
    }

    #[test]
    fn test_times_out() {
        initialize_test();

        let task_name = "times_out";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = Task::new(task_name, &task_path);
        task.time_limit = 0.5;

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
            assert!(!task.create_tests_for_cps());
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
            assert!(!task.create_tests_for_cps());
        }
    }

    #[test]
    fn create_with_custom_names() {
        initialize_test();

        let task_name = "tests_with_custom_names";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = Task::new(task_name, &task_path);

        task.get_input_file_name = Box::new(|test_id: i32, subtask_id: i32, test_id_in_subtask: i32| format!("in_{subtask_id}_{test_id_in_subtask}_{test_id}.txt"));
        task.get_output_file_name = Box::new(|test_id: i32, subtask_id: i32, test_id_in_subtask: i32| format!("out_{subtask_id}_{test_id_in_subtask}_{test_id}.txt"));

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
            assert!(task.create_tests_for_cps());
        }
    }

    #[test]
    fn create_with_custom_names2() {
        initialize_test();

        let task_name = "tests_with_custom_names2";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = Task::new(task_name, &task_path);

        task.get_input_file_name = Box::new(|_test_id: i32, subtask_id: i32, test_id_in_subtask: i32| format!("in_{subtask_id}_{test_id_in_subtask}.txt"));
        task.get_output_file_name = Box::new(|_test_id: i32, subtask_id: i32, test_id_in_subtask: i32| format!("out_{subtask_id}_{test_id_in_subtask}.txt"));

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
            assert!(task.create_tests_for_cps());
        }
    }

    #[test]
    fn create_with_custom_names3() {
        initialize_test();

        let task_name = "tests_with_custom_names3";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = Task::new(task_name, &task_path);

        task.get_input_file_name = Box::new(|test_id: i32, _subtask_id: i32, _test_id_in_subtask: i32| format!("in_{test_id}.txt"));
        task.get_output_file_name = Box::new(|test_id: i32, _subtask_id: i32, _test_id_in_subtask: i32| format!("out_{test_id}.txt"));

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
            assert!(task.create_tests_for_cps());
        }
    }
}
