#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod array_tests {
    use crate::tests::generic_tests::generic_tests::{initialize_test, TESTS_DIR};
    use crate::{array_generator, Subtask, Task};
    use anyhow::bail;
    use std::path::PathBuf;

    #[test]
    fn test_array_generator() {
        initialize_test();

        let task_name = "array_generator";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

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

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        // create subtasks
        let mut subtask1 = Subtask::new(20);

        subtask1.set_checker(|mut input| {
            let array = input.get_array()?;
            input.expect_end()?;
            let n = array.len();
            if !(1..=100).contains(&n) {
                bail!("n should be in range [1, 100]");
            }
            for x in array {
                if !(1..=100).contains(&x) {
                    bail!("all array values should be in range [1, 100]");
                }
            }
            Ok(())
        });

        subtask1.add_test(5, array_generator(1, 100, 1, 100));
        subtask1.add_test(5, array_generator(1, 100, 1, 1));
        subtask1.add_test(5, array_generator(100, 100, 1, 100));
        subtask1.add_test(5, array_generator(100, 100, 1, 1));
        subtask1.add_test(1, array_generator(100, 100, 1, 1));

        // n = 42
        let mut subtask2 = Subtask::new(40);

        subtask2.set_checker(|mut input| {
            let array = input.get_array()?;
            input.expect_end()?;
            let n = array.len();
            if n != 42 {
                bail!("n should be 42");
            }
            for x in array {
                if !(1..=100).contains(&x) {
                    bail!("all array values should be in range [1, 100]");
                }
            }
            Ok(())
        });

        subtask2.add_test(5, array_generator(42, 42, 1, 100));
        subtask2.add_test(5, array_generator(42, 42, 1, 1));
        subtask2.add_test(5, array_generator(42, 42, 100, 100));

        // all values are 47
        let mut subtask3 = Subtask::new(40);

        subtask3.set_checker(|mut input| {
            let array = input.get_array()?;
            input.expect_end()?;
            let n = array.len();
            if !(1..=100).contains(&n) {
                bail!("n should be in range [1, 100]");
            }
            for x in array {
                if x != 47 {
                    bail!("all array values should be 47");
                }
            }
            Ok(())
        });

        subtask3.add_test(5, array_generator(1, 100, 47, 47));
        subtask3.add_test(5, array_generator(100, 100, 47, 47));
        subtask3.add_test(5, array_generator(1, 1, 47, 47));

        let subtask1 = task.add_subtask(subtask1);
        let subtask2 = task.add_subtask(subtask2);
        task.add_subtask(subtask3);

        for _ in 0..10 {
            assert!(task.create_tests());
        }

        task.add_subtask_dependency(subtask2, subtask1);

        for _ in 0..10 {
            assert!(!task.create_tests());
        }
    }
}
