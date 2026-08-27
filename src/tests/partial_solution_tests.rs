#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod partial_solution_tests {
    use crate::Mode;
    use crate::array_generator;
    use crate::tests::generic_tests::generic_tests::Test;
    use crate::{Error, Subtask};

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

        task.task = task.task.with_solution_source(solution_contents);

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
        let subtask1 = crate::Subtask::new(0, "").with_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18
        let subtask2 = crate::Subtask::new(0, "").with_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        task.task = task
            .task
            .with_subtask(subtask1)
            .with_subtask(subtask2)
            // add partial solutions
            .with_partial_solution("partial", partial_solution_contents, &[0]);

        task.test();
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

        task.task = task.task.with_solution_source(solution_contents);

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
        let subtask1 = crate::Subtask::new(0, "").with_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18
        let subtask2 = crate::Subtask::new(0, "").with_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        task.task = task
            .task
            .with_subtask(subtask1)
            .with_subtask(subtask2)
            // add partial solutions
            .with_partial_solution("partial", partial_solution_contents, &[0]);

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

        task.task = task.task.with_solution_source(solution_contents);

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
        let subtask1 = crate::Subtask::new(0, "").with_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18
        let subtask2 = crate::Subtask::new(0, "").with_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        task.task = task
            .task
            .with_subtask(subtask1)
            .with_subtask(subtask2)
            // add partial solutions
            .with_partial_solution("partial", partial_solution_contents, &[]);

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

        task.task = task.task.with_solution_source(solution_contents);

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
        let subtask1 = crate::Subtask::new(0, "").with_test(5, array_generator(1, 100, 1, 100));

        // subtask 2, the sum is less than 10^18
        let subtask2 = crate::Subtask::new(0, "").with_test(5, array_generator(1, 100, 1, 1_000_000_000));

        // create subtasks
        task.task = task
            .task
            .with_subtask(subtask1)
            .with_subtask(subtask2)
            // add partial solutions
            .with_partial_solution("partial", partial_solution_contents, &[]);

        task.test();
    }

    /// The point of declaring the subtasks a partial solution passes is that the
    /// generated tests really do break it everywhere else. A partial solution
    /// that survives a subtask it was declared to fail means the test data is
    /// weaker than the declaration claims, so the run has to fail rather than
    /// hand out subtask scores nobody can trust.
    #[test]
    fn test_partial_solution_that_is_never_broken_is_reported() {
        let mut task = Test::new();

        let solution_contents = r#"
        #include <iostream>
        using namespace std;
        int main() {
            int n;
            cin>>n;
            cout<<n<<"\n";
            return 0;
        }
        "#;

        // Identical behaviour to the correct solution, so no generated test can
        // ever tell the two apart.
        let partial_solution_contents = r#"
        #include <iostream>
        using namespace std;
        int main() {
            int n;
            cin>>n;
            cout<<n<<"\n"; // same answer as the official solution
            return 0;
        }
        "#;

        task.task = task
            .task
            .with_solution_source(solution_contents)
            .with_subtask(Subtask::new(0, "first").with_test(2, |_rng| "1\n".to_owned()))
            .with_subtask(Subtask::new(0, "second").with_test(2, |_rng| "2\n".to_owned()))
            // Declared to pass only the first subtask, but it passes both.
            .with_partial_solution("indistinguishable", partial_solution_contents, &[0])
            .with_min_failures(1)
            .with_max_tries(3);

        assert!(matches!(
            task.task.run_mode(Mode::Files),
            Err(Error::PartialSolutionPassesExtraSubtask {
                subtask_number: 2,
                partial_number: 1,
                ..
            })
        ));
    }

    /// A subtask index that does not exist is a typo (1-based numbering is the
    /// usual one), and it would otherwise quietly turn into "this solution has to
    /// fail everywhere".
    #[test]
    fn test_partial_solution_with_unknown_subtask_index_is_rejected() {
        let mut task = Test::new();

        let solution_contents = "int main() { return 0; }";

        task.task = task
            .task
            .with_solution_source(solution_contents)
            .with_subtask(Subtask::new(0, "only subtask").with_test(1, |_rng| "1\n".to_owned()))
            .with_partial_solution("partial", solution_contents, &[1]);

        assert!(matches!(
            task.task.run_mode(Mode::Files),
            Err(Error::InvalidSubtaskIndex {
                subtask_number: 1,
                num_subtasks: 1,
                partial_number: 1,
                ..
            })
        ));
    }
}
