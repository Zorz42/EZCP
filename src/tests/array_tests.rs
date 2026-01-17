#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod array_tests {
    use crate::tests::generic_tests::generic_tests::Test;
    use crate::{Subtask, array_generator};

    #[test]
    fn test_array_generator() {
        let mut task = Test::new();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            int a[n];
            for(int i=0;i<n;i++) {
                cin>>a[i];
            }
            cout<<"1\n";
            return 0; 
        }
        
        "#;

        task.task = task.task.with_solution_source(solution_contents.to_owned());

        // create subtasks
        let subtask1 = Subtask::new()
            .with_test(5, array_generator(1, 100, 1, 100))
            .with_test(5, array_generator(1, 100, 1, 1))
            .with_test(5, array_generator(100, 100, 1, 100))
            .with_test(5, array_generator(100, 100, 1, 1))
            .with_test(1, array_generator(100, 100, 1, 1));

        // n = 42
        let subtask2 = Subtask::new()
            .with_test(5, array_generator(42, 42, 1, 100))
            .with_test(5, array_generator(42, 42, 1, 1))
            .with_test(5, array_generator(42, 42, 100, 100));

        // all values are 47
        let subtask3 = Subtask::new()
            .with_test(5, array_generator(1, 100, 47, 47))
            .with_test(5, array_generator(100, 100, 47, 47))
            .with_test(5, array_generator(1, 1, 47, 47));

        task.task = task.task.with_subtask(subtask1);
        task.task = task.task.with_subtask(subtask2);
        task.task = task.task.with_subtask(subtask3);

        task.task.run().unwrap();
    }
}
