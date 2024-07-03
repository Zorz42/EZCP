#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod partial_solution_tests {
    use crate::array_generator;
    use crate::tests::generic_tests::generic_tests::TESTS_DIR;
    use std::path::PathBuf;

    #[test]
    fn test_partial_solution() {
        let task_name = "partial_solution";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = crate::Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            long long sum=0;
            for(int i=0;i<n;i++) {
                int a;
                cin>>a;
                sum+=a;
            }
            cout<<sum<<"\n";
            return 0; 
        }
        "#;

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        // create partial solution file (it overflows)
        let partial_solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            int sum=0;
            for(int i=0;i<n;i++) {
                int a;
                cin>>a;
                sum+=a;
            }
            cout<<sum<<"\n";
            return 0; 
        }
        "#;

        std::fs::write(task_path.join("solution1.cpp"), partial_solution_contents).unwrap();

        // subtask 1, the sum is less than 10^6
        let mut subtask1 = crate::Subtask::new(50);

        subtask1.add_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18

        let mut subtask2 = crate::Subtask::new(50);
        subtask2.add_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        let subtask1 = task.add_subtask(subtask1);
        let subtask2 = task.add_subtask(subtask2);

        // add dependencies
        task.add_subtask_dependency(subtask2, subtask1);

        // add partial solutions
        task.add_partial_solution("solution1.cpp", &[subtask1]);

        for _ in 0..10 {
            assert!(task.create_tests());
        }
    }

    #[test]
    fn test_partial_solution_wrong_subtask() {
        let task_name = "partial_solution_wrong_subtask";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = crate::Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            long long sum=0;
            for(int i=0;i<n;i++) {
                int a;
                cin>>a;
                sum+=a;
            }
            cout<<sum<<"\n";
            return 0; 
        }
        "#;

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        // create partial solution file (it overflows)
        let partial_solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            int sum=0;
            for(int i=0;i<n;i++) {
                int a;
                cin>>a;
                sum+=a;
            }
            cout<<sum<<"\n";
            return 0; 
        }
        "#;

        std::fs::write(task_path.join("solution1.cpp"), partial_solution_contents).unwrap();

        // subtask 1, the sum is less than 10^6
        let mut subtask1 = crate::Subtask::new(50);

        subtask1.add_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18

        let mut subtask2 = crate::Subtask::new(50);
        subtask2.add_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        let subtask1 = task.add_subtask(subtask1);
        let subtask2 = task.add_subtask(subtask2);

        // add dependencies
        task.add_subtask_dependency(subtask2, subtask1);

        // add partial solutions
        task.add_partial_solution("solution1.cpp", &[subtask2]);

        for _ in 0..10 {
            assert!(!task.create_tests());
        }
    }

    #[test]
    fn test_partial_solution_wa() {
        let task_name = "partial_solution_wa";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = crate::Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            long long sum=0;
            for(int i=0;i<n;i++) {
                int a;
                cin>>a;
                sum+=a;
            }
            cout<<sum<<"\n";
            return 0; 
        }
        "#;

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        // create partial solution file (it overflows)
        let partial_solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            int sum=0;
            for(int i=0;i<n;i++) {
                int a;
                cin>>a;
                sum+=a;
            }
            cout<<sum+1<<"\n";
            return 0; 
        }
        "#;

        std::fs::write(task_path.join("solution1.cpp"), partial_solution_contents).unwrap();

        // subtask 1, the sum is less than 10^6
        let mut subtask1 = crate::Subtask::new(50);

        subtask1.add_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18

        let mut subtask2 = crate::Subtask::new(50);
        subtask2.add_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        let subtask1 = task.add_subtask(subtask1);
        let subtask2 = task.add_subtask(subtask2);

        // add dependencies
        task.add_subtask_dependency(subtask2, subtask1);

        // add partial solutions
        task.add_partial_solution("solution1.cpp", &[subtask1]);

        for _ in 0..10 {
            assert!(!task.create_tests());
        }
    }

    #[test]
    fn test_partial_solution_tle() {
        let task_name = "partial_solution_tle";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = crate::Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            long long sum=0;
            for(int i=0;i<n;i++) {
                int a;
                cin>>a;
                sum+=a;
            }
            cout<<sum<<"\n";
            return 0; 
        }
        "#;

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        // create partial solution file (it overflows)
        let partial_solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            int sum=0;
            for(int i=0;i<n;i++) {
                int a;
                cin>>a;
                while(a--)
                    sum++;
            }
            cout<<sum<<"\n";
            return 0; 
        }
        "#;

        std::fs::write(task_path.join("solution1.cpp"), partial_solution_contents).unwrap();

        // subtask 1, the sum is less than 10^6
        let mut subtask1 = crate::Subtask::new(50);

        subtask1.add_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18

        let mut subtask2 = crate::Subtask::new(50);
        subtask2.add_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        let subtask1 = task.add_subtask(subtask1);
        let subtask2 = task.add_subtask(subtask2);

        // add dependencies
        task.add_subtask_dependency(subtask2, subtask1);

        // add partial solutions
        task.add_partial_solution("solution1.cpp", &[subtask1]);

        for _ in 0..10 {
            assert!(task.create_tests());
        }
    }

    #[test]
    fn test_partial_solution_crash() {
        let task_name = "partial_solution_crash";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = crate::Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            long long sum=0;
            for(int i=0;i<n;i++) {
                int a;
                cin>>a;
                sum+=a;
            }
            cout<<sum<<"\n";
            return 0; 
        }
        "#;

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        // create partial solution file (it crashes)
        let partial_solution_contents = "
        int main() {
            int*n=nullptr;
            while(true){
                *n=1;
                n++;
            }
            return 0; 
        }
        ";

        std::fs::write(task_path.join("solution1.cpp"), partial_solution_contents).unwrap();

        // subtask 1, the sum is less than 10^6
        let mut subtask1 = crate::Subtask::new(50);

        subtask1.add_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18

        let mut subtask2 = crate::Subtask::new(50);
        subtask2.add_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        let subtask1 = task.add_subtask(subtask1);
        let subtask2 = task.add_subtask(subtask2);

        // add dependencies
        task.add_subtask_dependency(subtask2, subtask1);

        // add partial solutions
        task.add_partial_solution("solution1.cpp", &[]);

        for _ in 0..10 {
            assert!(task.create_tests());
        }
    }

    #[test]
    fn test_partial_solution_tle2() {
        let task_name = "partial_solution_tle2";
        let task_path = PathBuf::from(TESTS_DIR).join(task_name);
        let mut task = crate::Task::new(task_name, &task_path);

        // create directory
        std::fs::create_dir_all(task_path.clone()).unwrap();

        // create solution file
        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            long long sum=0;
            for(int i=0;i<n;i++) {
                int a;
                cin>>a;
                sum+=a;
            }
            cout<<sum<<"\n";
            return 0; 
        }
        "#;

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();

        // create partial solution file (it overflows)
        let partial_solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            int sum=0;
            for(int i=0;i<n;i++) {
                int a;
                cin>>a;
                while(a++)
                    sum++;
            }
            cout<<sum<<"\n";
            return 0; 
        }
        "#;

        std::fs::write(task_path.join("solution1.cpp"), partial_solution_contents).unwrap();

        // subtask 1, the sum is less than 10^6
        let mut subtask1 = crate::Subtask::new(50);

        subtask1.add_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18

        let mut subtask2 = crate::Subtask::new(50);
        subtask2.add_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        let subtask1 = task.add_subtask(subtask1);
        let subtask2 = task.add_subtask(subtask2);

        // add dependencies
        task.add_subtask_dependency(subtask2, subtask1);

        // add partial solutions
        task.add_partial_solution("solution1.cpp", &[subtask1]);

        for _ in 0..10 {
            assert!(!task.create_tests());
        }
    }
}
