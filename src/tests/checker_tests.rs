#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod generator_tests {
    use crate::tests::generic_tests::generic_tests::Test;
    use crate::{array_generator, Error, Subtask};

    #[test]
    fn test_checker() {
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
        let mut subtask1 = Subtask::new(20);

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
        subtask1.add_test(5, array_generator(1, 100, 100, 100));
        subtask1.add_test(5, array_generator(1, 1, 1, 100));
        subtask1.add_test(5, array_generator(100, 100, 100, 100));
        subtask1.add_test(5, array_generator(1, 1, 100, 100));
        subtask1.add_test(5, array_generator(100, 100, 1, 1));
        subtask1.add_test(5, array_generator(1, 1, 1, 1));

        task.task.add_subtask(subtask1);

        task.test()
    }

    #[test]
    fn test_checker_fail() {
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
        let mut subtask1 = Subtask::new(20);

        subtask1.set_checker(|mut input| {
            let array = input.get_array()?;
            input.expect_end()?;
            let n = array.len();
            if !(1..=100).contains(&n) {
                crate::bail!("n should be in range [1, 100]");
            }
            for x in array {
                if !(1..=99).contains(&x) {
                    crate::bail!("all array values should be in range [1, 99]");
                }
            }
            Ok(())
        });

        subtask1.add_test(5, array_generator(1, 100, 1, 100));
        subtask1.add_test(5, array_generator(1, 100, 1, 1));
        subtask1.add_test(5, array_generator(100, 100, 1, 100));
        subtask1.add_test(5, array_generator(1, 100, 100, 100));
        subtask1.add_test(5, array_generator(1, 1, 1, 100));
        subtask1.add_test(5, array_generator(100, 100, 100, 100));
        subtask1.add_test(5, array_generator(1, 1, 100, 100));
        subtask1.add_test(5, array_generator(100, 100, 1, 1));
        subtask1.add_test(5, array_generator(1, 1, 1, 1));

        task.task.add_subtask(subtask1);

        for _ in 0..10 {
            assert!(matches!(task.task.create_tests().unwrap_err(), Error::CustomError { .. }));
        }
    }

    #[test]
    fn test_checker2() {
        let mut task = Test::new();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int a,b;
            cin>>a>>b;
            cout<<a+b<<"\n";
            return 0; 
        }
        
        "#;

        task.create_solution(solution_contents);

        // a = b
        let mut subtask1 = Subtask::new(20);

        subtask1.set_checker(|mut input| {
            let a = input.get_int()?;
            let b = input.get_int()?;
            input.expect_end()?;
            if a != b {
                crate::bail!("a should be equal to b");
            }
            if !(1..=100).contains(&a) {
                crate::bail!("a and b should be in range [1, 100]");
            }
            Ok(())
        });

        subtask1.add_test_str("1 1");
        subtask1.add_test_str("2 2");
        subtask1.add_test_str("3 3");

        task.task.add_subtask(subtask1);

        task.test()
    }

    #[test]
    fn test_checker_fail2() {
        let mut task = Test::new();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int a,b;
            cin>>a>>b;
            cout<<a+b<<"\n";
            return 0; 
        }
        
        "#;

        task.create_solution(solution_contents);

        // a = b
        let mut subtask1 = Subtask::new(20);

        subtask1.set_checker(|mut input| {
            let a = input.get_int()?;
            let b = input.get_int()?;
            input.expect_end()?;
            if a != b {
                crate::bail!("a should be equal to b");
            }
            if !(1..=100).contains(&a) {
                crate::bail!("a and b should be in range [1, 100]");
            }
            Ok(())
        });

        subtask1.add_test_str("1 1");
        subtask1.add_test_str("2 3");
        subtask1.add_test_str("3 3");

        task.task.add_subtask(subtask1);

        for _ in 0..10 {
            assert!(matches!(task.task.create_tests().unwrap_err(), Error::CustomError { .. }));
        }
    }

    #[test]
    fn test_checker_fail3() {
        let mut task = Test::new();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int a,b;
            cin>>a>>b;
            cout<<a+b<<"\n";
            return 0; 
        }
        
        "#;

        task.create_solution(solution_contents);

        // a = b
        let mut subtask1 = Subtask::new(20);

        subtask1.set_checker(|mut input| {
            let a = input.get_int()?;
            let b = input.get_int()?;
            input.expect_end()?;
            if a != b {
                crate::bail!("a should be equal to b");
            }
            if !(1..=100).contains(&a) {
                crate::bail!("a and b should be in range [1, 100]");
            }
            Ok(())
        });

        subtask1.add_test_str("1 1");
        subtask1.add_test_str("2 2");
        subtask1.add_test_str("3");
        subtask1.add_test_str("5 5");

        task.task.add_subtask(subtask1);

        for _ in 0..10 {
            assert!(matches!(task.task.create_tests().unwrap_err(), Error::InputExpectedInteger { .. }));
        }
    }

    #[test]
    fn test_checker_fail4() {
        let mut task = Test::new();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int a,b;
            cin>>a>>b;
            cout<<a+b<<"\n";
            return 0; 
        }
        
        "#;

        task.create_solution(solution_contents);

        // a = b
        let mut subtask1 = Subtask::new(20);

        subtask1.set_checker(|mut input| {
            let a = input.get_int()?;
            let b = input.get_int()?;
            input.expect_end()?;
            if a != b {
                crate::bail!("a should be equal to b");
            }
            if !(1..=100).contains(&a) {
                crate::bail!("a and b should be in range [1, 100]");
            }
            Ok(())
        });

        subtask1.add_test_str("1 1");
        subtask1.add_test_str("2 2");
        subtask1.add_test_str("3 3 3");
        subtask1.add_test_str("5 5");

        task.task.add_subtask(subtask1);

        for _ in 0..10 {
            assert!(matches!(task.task.create_tests().unwrap_err(), Error::InputExpectedEnd { .. }));
        }
    }

    #[test]
    fn test_checker3() {
        let mut task = Test::new();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int a,b;
            cin>>a>>b;
            cout<<a+b<<"\n";
            return 0; 
        }
        
        "#;

        task.create_solution(solution_contents);

        // a = b
        let mut subtask1 = Subtask::new(20);

        subtask1.set_checker(|mut input| {
            let a = input.get_int()?;
            let b = input.get_int()?;
            input.expect_end()?;
            if a != b {
                crate::bail!("a should be equal to b");
            }
            if !(1..=100).contains(&a) {
                crate::bail!("a and b should be in range [1, 100]");
            }
            Ok(())
        });

        subtask1.add_test_str("1 1");
        subtask1.add_test_str("2 2");
        subtask1.add_test_str("3 3");

        let mut subtask2 = Subtask::new(80);
        subtask2.set_checker(|mut input| {
            let a = input.get_int()?;
            let b = input.get_int()?;
            input.expect_end()?;
            if !(1..=100).contains(&a) {
                crate::bail!("a should be in range [1, 100]");
            }
            if !(1..=100).contains(&b) {
                crate::bail!("b should be in range [1, 100]");
            }
            Ok(())
        });

        subtask2.add_test_str("1 1");
        subtask2.add_test_str("2 4");
        subtask2.add_test_str("3 5");
        subtask2.add_test_str("4 6");
        subtask2.add_test_str("100 100");

        // b == 1
        let mut subtask3 = Subtask::new(100);
        subtask3.set_checker(|mut input| {
            let a = input.get_int()?;
            let b = input.get_int()?;
            input.expect_end()?;
            if b != 1 {
                crate::bail!("b should be equal to 1");
            }
            if !(1..=100).contains(&a) {
                crate::bail!("a should be in range [1, 100]");
            }
            Ok(())
        });

        subtask3.add_test_str("1 1");
        subtask3.add_test_str("2 1");
        subtask3.add_test_str("3 1");
        subtask3.add_test_str("4 1");
        subtask3.add_test_str("100 1");

        let subtask1 = task.task.add_subtask(subtask1);
        let subtask2 = task.task.add_subtask(subtask2);
        task.task.add_subtask(subtask3);

        task.task.add_subtask_dependency(subtask2, subtask1);

        task.test()
    }

    #[test]
    fn test_checker_fail5() {
        let mut task = Test::new();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int a,b;
            cin>>a>>b;
            cout<<a+b<<"\n";
            return 0; 
        }
        
        "#;

        task.create_solution(solution_contents);

        // a = b
        let mut subtask1 = Subtask::new(20);

        subtask1.set_checker(|mut input| {
            let a = input.get_int()?;
            let b = input.get_int()?;
            input.expect_end()?;
            if a != b {
                crate::bail!("a should be equal to b");
            }
            if !(1..=100).contains(&a) {
                crate::bail!("a and b should be in range [1, 100]");
            }
            Ok(())
        });

        subtask1.add_test_str("1 1");
        subtask1.add_test_str("2 2");
        subtask1.add_test_str("3 3");

        let mut subtask2 = Subtask::new(80);
        subtask2.set_checker(|mut input| {
            let a = input.get_int()?;
            let b = input.get_int()?;
            input.expect_end()?;
            if !(1..=100).contains(&a) {
                crate::bail!("a should be in range [1, 100]");
            }
            if !(1..=100).contains(&b) {
                crate::bail!("b should be in range [1, 100]");
            }
            Ok(())
        });

        subtask2.add_test_str("1 1");
        subtask2.add_test_str("2 4");
        subtask2.add_test_str("3 5");
        subtask2.add_test_str("4 6");
        subtask2.add_test_str("100 100");

        // b == 1
        let mut subtask3 = Subtask::new(100);
        subtask3.set_checker(|mut input| {
            let a = input.get_int()?;
            let b = input.get_int()?;
            input.expect_end()?;
            if b != 1 {
                crate::bail!("b should be equal to 1");
            }
            if !(1..=100).contains(&a) {
                crate::bail!("a should be in range [1, 100]");
            }
            Ok(())
        });

        subtask3.add_test_str("1 1");
        subtask3.add_test_str("2 1");
        subtask3.add_test_str("3 1");
        subtask3.add_test_str("4 1");
        subtask3.add_test_str("100 1");

        task.task.add_subtask(subtask1);
        let subtask2 = task.task.add_subtask(subtask2);
        let subtask3 = task.task.add_subtask(subtask3);

        for _ in 0..10 {
            task.task.create_tests().unwrap();
        }

        task.task.add_subtask_dependency(subtask3, subtask2);

        for _ in 0..10 {
            assert!(matches!(task.task.create_tests().unwrap_err(), Error::CustomError { .. }));
        }
    }
}
