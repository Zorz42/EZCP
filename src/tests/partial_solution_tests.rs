#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod partial_solution_tests {
    use crate::array_generator;
    use crate::tests::generic_tests::generic_tests::Test;

    #[test]
    fn test_partial_solution() {
        let mut task = Test::new();

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

        task.task = task.task.with_solution_source(solution_contents.to_owned());

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

        // subtask 1, the sum is less than 10^6
        let subtask1 = crate::Subtask::new().with_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18
        let subtask2 = crate::Subtask::new().with_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        task.task = task
            .task
            .with_subtask(subtask1)
            .with_subtask(subtask2)
            // add partial solutions
            .with_solution(partial_solution_contents.to_owned(), &[0]);

        task.test();
    }

    #[test]
    fn test_partial_solution_wrong_subtask() {
        let mut task = Test::new();

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

        task.task = task.task.with_solution_source(solution_contents.to_owned());

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

        // subtask 1, the sum is less than 10^6
        let subtask1 = crate::Subtask::new().with_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18
        let subtask2 = crate::Subtask::new().with_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        task.task = task
            .task
            .with_subtask(subtask1)
            .with_subtask(subtask2)
            // add partial solutions - intentionally wrong expected subtask
            .with_solution(partial_solution_contents.to_owned(), &[1]);

        assert!(task.task.run().is_err());
    }

    #[test]
    fn test_partial_solution_wa() {
        let mut task = Test::new();

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

        task.task = task.task.with_solution_source(solution_contents.to_owned());

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

        // subtask 1, the sum is less than 10^6
        let subtask1 = crate::Subtask::new().with_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18
        let subtask2 = crate::Subtask::new().with_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        task.task = task
            .task
            .with_subtask(subtask1)
            .with_subtask(subtask2)
            // add partial solutions
            .with_solution(partial_solution_contents.to_owned(), &[0]);

        assert!(task.task.run().is_err());
    }

    #[test]
    fn test_partial_solution_tle() {
        let mut task = Test::new();

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

        task.task = task.task.with_solution_source(solution_contents.to_owned());

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

        // subtask 1, the sum is less than 10^6
        let subtask1 = crate::Subtask::new().with_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18
        let subtask2 = crate::Subtask::new().with_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        task.task = task
            .task
            .with_subtask(subtask1)
            .with_subtask(subtask2)
            // add partial solutions
            .with_solution(partial_solution_contents.to_owned(), &[0]);

        task.test();
    }

    #[test]
    fn test_partial_solution_crash() {
        let mut task = Test::new();

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

        task.task = task.task.with_solution_source(solution_contents.to_owned());

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

        // subtask 1, the sum is less than 10^6
        let subtask1 = crate::Subtask::new().with_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18
        let subtask2 = crate::Subtask::new().with_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        task.task = task
            .task
            .with_subtask(subtask1)
            .with_subtask(subtask2)
            // add partial solutions
            .with_solution(partial_solution_contents.to_owned(), &[]);

        task.test();
    }

    #[test]
    fn test_partial_solution_tle2() {
        let mut task = Test::new();

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

        task.task = task.task.with_solution_source(solution_contents.to_owned());

        // create partial solution file (it overflows)
        let partial_solution_contents = r#"
        #include <iostream>
        using namespace std;
        
        int main() {
            int n;
            cin>>n;
            int sum=0;
            for(int i=0;i<n;i++) {
                long long a;
                cin>>a;
                while(a++)
                    sum++;
            }
            cout<<sum<<"\n";
            return 0; 
        }
        "#;

        // subtask 1, the sum is less than 10^6
        let subtask1 = crate::Subtask::new().with_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18
        let subtask2 = crate::Subtask::new().with_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        task.task = task
            .task
            .with_subtask(subtask1)
            .with_subtask(subtask2)
            // add partial solutions
            .with_solution(partial_solution_contents.to_owned(), &[]);

        task.test();
    }
}
