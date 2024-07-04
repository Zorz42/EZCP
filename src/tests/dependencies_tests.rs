#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod dependencies_tests {
    use crate::tests::generic_tests::generic_tests::{initialize_test, TESTS_DIR};
    use crate::{Subtask, Task};
    use std::path::PathBuf;

    #[test]
    fn create_with_dependencies() {
        initialize_test();

        let task_name = "dependencies";
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

        for _ in 0..10 {
            task.create_tests().unwrap();
        }
    }

    #[test]
    fn test_complicated_dependencies() {
        initialize_test();

        let task_name = "complicated_dependencies";
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
        subtask1.add_test_str("1 1\n");
        subtask1.add_test_str("1 2\n");
        subtask1.add_test_str("1 3\n");
        let mut subtask2 = Subtask::new(20);
        subtask2.add_test_str("2 1\n");
        subtask2.add_test_str("2 2\n");
        subtask2.add_test_str("2 3\n");
        let mut subtask3 = Subtask::new(20);
        subtask3.add_test_str("3 1\n");
        subtask3.add_test_str("3 2\n");
        subtask3.add_test_str("3 3\n");
        let mut subtask4 = Subtask::new(20);
        subtask4.add_test_str("4 1\n");
        subtask4.add_test_str("4 2\n");
        subtask4.add_test_str("4 3\n");
        let mut subtask5 = Subtask::new(20);
        subtask5.add_test_str("5 1\n");
        subtask5.add_test_str("5 2\n");
        subtask5.add_test_str("5 3\n");

        // create subtasks
        let subtask1 = task.add_subtask(subtask1);
        let subtask2 = task.add_subtask(subtask2);
        let subtask3 = task.add_subtask(subtask3);
        let subtask4 = task.add_subtask(subtask4);
        let subtask5 = task.add_subtask(subtask5);

        task.add_subtask_dependency(subtask2, subtask1);
        task.add_subtask_dependency(subtask3, subtask1);
        task.add_subtask_dependency(subtask3, subtask2);
        task.add_subtask_dependency(subtask4, subtask1);
        task.add_subtask_dependency(subtask4, subtask2);
        task.add_subtask_dependency(subtask4, subtask3);
        task.add_subtask_dependency(subtask5, subtask1);
        task.add_subtask_dependency(subtask5, subtask2);
        task.add_subtask_dependency(subtask5, subtask3);
        task.add_subtask_dependency(subtask5, subtask4);

        for _ in 0..10 {
            task.create_tests().unwrap();
        }
    }
}
