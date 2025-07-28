#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod runner_tests {
    use std::time::Instant;
    use tempfile::TempDir;
    use crate::Error::CompilerError;
    use crate::runner::cpp_runner::CppRunner;
    use crate::runner::runner::RunResult;
    use crate::tests::test_shared::initialize_logger;

    const HELLO_WORLD_PROGRAM: &str = r#"
    #include <iostream>
    using namespace std;
    int main() {
        cout << "Hello, World!" << endl;
        return 0;
    }
    "#;

    #[test]
    fn test_runner_new() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();

        let _runner = CppRunner::new(tempdir.path().to_owned()).unwrap();

        drop(tempdir);
    }

    #[test]
    fn test_runner_add_program() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path().to_owned()).unwrap();

        let _handle = runner.add_program(HELLO_WORLD_PROGRAM).unwrap();

        drop(tempdir);
    }

    #[test]
    fn test_runner_add_faulty_program() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path().to_owned()).unwrap();

        let faulty_program_source = r#"
        #include <iostream>
        using namespace std;
        int main() {
            compile error here
            cout << "Hello, World!" << endl;
            return 1;
        }
        "#;

        assert!(matches!(runner.add_program(faulty_program_source), Err(CompilerError { .. })));

        drop(tempdir);
    }

    #[test]
    fn test_runner_run_program() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path().to_owned()).unwrap();

        let program_handle = runner.add_program(HELLO_WORLD_PROGRAM).unwrap();
        let task_handle = runner.add_task(program_handle, "".to_owned(), 1.0).unwrap();

        runner.run_tasks(None).unwrap();

        let result = runner.get_result(task_handle);

        assert!(matches!(result, RunResult::Ok( .. )));

        if let RunResult::Ok(_, output) = result {
            assert_eq!(output.trim(), "Hello, World!");
        }

        drop(tempdir);
    }

    #[test]
    fn test_runner_run_programs() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path().to_owned()).unwrap();

        let mut program_handles = Vec::new();
        for _ in 0..5 {
            let code = r#"
            #include <iostream>
            using namespace std;
            int main() {
                int n;
                cin>>n;
                cout << $program_id$ << " " << n << endl;
                return 0;
            }
            "#.replace("$program_id$", &format!("{}", program_handles.len()));
            let program_handle = runner.add_program(&code).unwrap();
            program_handles.push(program_handle);
        }

        let mut task_handles = Vec::new();
        for i in 0..100 {
            let input = format!("{}\n", i);
            let task_handle = runner.add_task(program_handles[i % program_handles.len()], input, 1.0).unwrap();
            task_handles.push(task_handle);
        }

        runner.run_tasks(None).unwrap();

        for (i, task_handle) in task_handles.iter().enumerate() {
            let result = runner.get_result(*task_handle);
            assert!(matches!(result, RunResult::Ok( .. )));

            if let RunResult::Ok(_, output) = result {
                assert_eq!(output.trim(), format!("{} {}", i % program_handles.len(), i));
            }
        }

        drop(tempdir);
    }

    #[test]
    fn test_same_program_100_times() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path().to_owned()).unwrap();

        let time = Instant::now();

        for _i in 0..100 {
            let _program_handle = runner.add_program(HELLO_WORLD_PROGRAM).unwrap();
        }

        // make sure it doesn't take too long
        assert!(time.elapsed().as_secs() < 10, "Adding the same program 100 times took too long");

        drop(tempdir);
    }
}