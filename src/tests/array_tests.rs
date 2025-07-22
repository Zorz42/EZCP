#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod array_tests {
    use crate::{array_generator, Error, Subtask};
    use crate::tests::generic_tests::generic_tests::Test;

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

        task.create_solution(solution_contents);

        // create subtasks
        let mut subtask1 = Subtask::new();

        subtask1.set_checker(|mut input| {
            let array = input.get_array()?;
            input.expect_end()?;
            let n = array.len();
            if !(1..=100).contains(&n) {
                crate::bail!("n should be in range [1, 100]");
            }
            for x in array {
                if !(1..=100).contains(&x) {
                    crate::bail!("all array values should be in range [1, 100]");
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
        let mut subtask2 = Subtask::new();

        subtask2.set_checker(|mut input| {
            let array = input.get_array()?;
            input.expect_end()?;
            let n = array.len();
            if n != 42 {
                crate::bail!("n should be 42");
            }
            for x in array {
                if !(1..=100).contains(&x) {
                    crate::bail!("all array values should be in range [1, 100]");
                }
            }
            Ok(())
        });

        subtask2.add_test(5, array_generator(42, 42, 1, 100));
        subtask2.add_test(5, array_generator(42, 42, 1, 1));
        subtask2.add_test(5, array_generator(42, 42, 100, 100));

        // all values are 47
        let mut subtask3 = Subtask::new();

        subtask3.set_checker(|mut input| {
            let array = input.get_array()?;
            input.expect_end()?;
            let n = array.len();
            if !(1..=100).contains(&n) {
                crate::bail!("n should be in range [1, 100]");
            }
            for x in array {
                if x != 47 {
                    crate::bail!("all array values should be 47");
                }
            }
            Ok(())
        });

        subtask3.add_test(5, array_generator(1, 100, 47, 47));
        subtask3.add_test(5, array_generator(100, 100, 47, 47));
        subtask3.add_test(5, array_generator(1, 1, 47, 47));

        let subtask1 = task.task.add_subtask(subtask1);
        let subtask2 = task.task.add_subtask(subtask2);
        task.task.add_subtask(subtask3);

        for _ in 0..10 {
            task.task.create_tests().unwrap();
        }

        task.task.add_subtask_dependency(subtask2, subtask1);

        for _ in 0..10 {
            assert!(matches!(task.task.create_tests().unwrap_err(), Error::CustomError { .. }));
        }
    }
}
