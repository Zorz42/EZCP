#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod dependencies_tests {
    use crate::tests::generic_tests::generic_tests::Test;
    use crate::{Subtask};

    #[test]
    fn create_with_dependencies() {
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
        subtask2.add_test_str("4\n");
        subtask2.add_test_str("5\n");
        subtask2.add_test_str("6\n");
        let mut subtask3 = Subtask::new();
        subtask3.add_test_str("7\n");
        subtask3.add_test_str("8\n");
        subtask3.add_test_str("9\n");

        // create subtasks
        let subtask1 = task.task.add_subtask(subtask1);
        let subtask2 = task.task.add_subtask(subtask2);
        let subtask3 = task.task.add_subtask(subtask3);

        task.task.add_subtask_dependency(subtask3, subtask1);
        task.task.add_subtask_dependency(subtask3, subtask2);

        task.test()
    }

    #[test]
    fn test_complicated_dependencies() {
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
        subtask1.add_test_str("1 1\n");
        subtask1.add_test_str("1 2\n");
        subtask1.add_test_str("1 3\n");
        let mut subtask2 = Subtask::new();
        subtask2.add_test_str("2 1\n");
        subtask2.add_test_str("2 2\n");
        subtask2.add_test_str("2 3\n");
        let mut subtask3 = Subtask::new();
        subtask3.add_test_str("3 1\n");
        subtask3.add_test_str("3 2\n");
        subtask3.add_test_str("3 3\n");
        let mut subtask4 = Subtask::new();
        subtask4.add_test_str("4 1\n");
        subtask4.add_test_str("4 2\n");
        subtask4.add_test_str("4 3\n");
        let mut subtask5 = Subtask::new();
        subtask5.add_test_str("5 1\n");
        subtask5.add_test_str("5 2\n");
        subtask5.add_test_str("5 3\n");

        // create subtasks
        let subtask1 = task.task.add_subtask(subtask1);
        let subtask2 = task.task.add_subtask(subtask2);
        let subtask3 = task.task.add_subtask(subtask3);
        let subtask4 = task.task.add_subtask(subtask4);
        let subtask5 = task.task.add_subtask(subtask5);

        task.task.add_subtask_dependency(subtask2, subtask1);
        task.task.add_subtask_dependency(subtask3, subtask1);
        task.task.add_subtask_dependency(subtask3, subtask2);
        task.task.add_subtask_dependency(subtask4, subtask1);
        task.task.add_subtask_dependency(subtask4, subtask2);
        task.task.add_subtask_dependency(subtask4, subtask3);
        task.task.add_subtask_dependency(subtask5, subtask1);
        task.task.add_subtask_dependency(subtask5, subtask2);
        task.task.add_subtask_dependency(subtask5, subtask3);
        task.task.add_subtask_dependency(subtask5, subtask4);

        task.test()
    }
}
