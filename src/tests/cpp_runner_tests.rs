#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod cpp_runner_tests {
    use crate::Error::CompilerError;
    use crate::runner::cpp_runner::CppRunner;
    use crate::runner::exec_runner::RunResult;
    use crate::tests::test_shared::initialize_logger;
    use std::fmt::Write as _;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    /// Runs `body` on a helper thread and fails the test if it does not finish in
    /// time, so that a regression shows up as a failure instead of a test run
    /// that hangs until CI kills it.
    #[allow(clippy::panic)]
    fn run_within<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(timeout: Duration, what: &str, body: F) -> T {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(body());
        });
        receiver.recv_timeout(timeout).unwrap_or_else(|_| panic!("{what} did not finish within {timeout:?}"))
    }

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

        let _runner = CppRunner::new(tempdir.path()).unwrap();

        drop(tempdir);
    }

    #[test]
    fn test_runner_add_program() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path()).unwrap();

        let _handle = runner.add_program(HELLO_WORLD_PROGRAM).unwrap();

        drop(tempdir);
    }

    #[test]
    fn test_runner_add_faulty_program() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path()).unwrap();

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
        let mut runner = CppRunner::new(tempdir.path()).unwrap();

        let program_handle = runner.add_program(HELLO_WORLD_PROGRAM).unwrap();
        let result = &runner.check_programs("", &[program_handle], 1000).unwrap()[0];

        assert!(matches!(result, RunResult::Ok(..)));

        if let RunResult::Ok(_, output) = result {
            assert_eq!(output.trim(), "Hello, World!");
        }

        drop(tempdir);
    }

    #[test]
    fn test_runner_run_programs() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path()).unwrap();

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
            "#
            .replace("$program_id$", &format!("{}", program_handles.len()));
            let program_handle = runner.add_program(&code).unwrap();
            program_handles.push(program_handle);
        }

        for i in 0..20 {
            let input = format!("{i}\n");
            let results = runner.check_programs(&input, &program_handles, 1000).unwrap();

            for (j, result) in results.iter().enumerate() {
                assert!(matches!(result, RunResult::Ok(..)));

                if let RunResult::Ok(_, output) = result {
                    assert_eq!(output.trim(), format!("{j} {i}"));
                }
            }
        }

        drop(tempdir);
    }

    #[test]
    fn test_same_program_100_times() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path()).unwrap();

        let time = Instant::now();

        for _i in 0..100 {
            let _program_handle = runner.add_program(HELLO_WORLD_PROGRAM).unwrap();
        }

        // make sure it doesn't take too long
        assert!(time.elapsed().as_secs() < 10, "Adding the same program 100 times took too long");

        drop(tempdir);
    }

    #[test]
    fn test_runner_program_tle() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path()).unwrap();

        let program_source = "
        int main() {
            while (true) {
                // Infinite loop to simulate TLE
            }
            return 0;
        }
        ";

        let program_handle = runner.add_program(program_source).unwrap();
        let result = &runner.check_programs("1\n", &[program_handle], 1000).unwrap()[0];

        // Check that the result is indeed a TLE
        assert!(matches!(result, RunResult::TimedOut));

        drop(tempdir);
    }

    #[test]
    #[cfg(not(windows))]
    fn test_runner_program_crash() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path()).unwrap();

        let program_source = "
        #include <signal.h>
        int main() {
            // Force a deterministic crash via SIGSEGV
            raise(SIGSEGV);
            return 0;
        }
        ";

        let program_handle = runner.add_program(program_source).unwrap();
        let result = &runner.check_programs("1\n", &[program_handle], 1000).unwrap()[0];

        // Check that the result is indeed a crash
        assert!(matches!(result, RunResult::Crashed));

        drop(tempdir);
    }

    #[test]
    #[cfg(windows)]
    fn test_runner_program_crash_windows() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path()).unwrap();

        // Force access violation by writing through null pointer, should crash
        let program_source = "
        #include <windows.h>
        int main() {
            int* p = 0;
            *p = 1;
            return 0;
        }
        ";

        let program_handle = runner.add_program(program_source).unwrap();
        let task_handle = runner.add_task(program_handle, Arc::from(""), 1000);

        runner.run_tasks(None, false).unwrap();

        let result = runner.get_result(task_handle);
        assert!(matches!(result, RunResult::Crashed));

        drop(tempdir);
    }

    /// A solution is free to stop reading once it has what it needs, which stalls
    /// EZCP on a full input pipe until the solution (and the timer holding the
    /// other read end) goes away.
    #[test]
    fn test_solution_that_ignores_most_of_its_input() {
        initialize_logger();

        let program_source = "
        #include <iostream>
        int main() {
            long long n;
            std::cin >> n;
            std::cout << n << std::endl;
            return 0;
        }
        ";

        // Comfortably larger than the pipe buffer on every supported platform.
        let mut input = String::from("7\n");
        for i in 0..500_000 {
            write!(input, "{i} ").unwrap();
        }

        let result = run_within(Duration::from_secs(90), "running a solution that ignores its input", move || {
            let tempdir = TempDir::new().unwrap();
            let mut runner = CppRunner::new(tempdir.path()).unwrap();
            let program_handle = runner.add_program(program_source).unwrap();
            runner.check_programs(&input, &[program_handle], 5000).unwrap().remove(0)
        });

        assert!(matches!(result, RunResult::Ok(..)), "Expected OK but got {result:?}");
        if let RunResult::Ok(_, output) = result {
            assert_eq!(output.trim(), "7");
        }
    }

    /// Reading a large input while writing a large output means neither side may
    /// block waiting for the other.
    #[test]
    fn test_large_input_and_large_output() {
        initialize_logger();

        let program_source = r#"
        #include <iostream>
        int main() {
            std::ios_base::sync_with_stdio(false);
            long long sum = 0, x;
            while (std::cin >> x) { sum += x; std::cout << x << "\n"; }
            std::cout << "sum " << sum << "\n";
            return 0;
        }
        "#;

        let count = 200_000_i64;
        let mut input = String::new();
        for i in 0..count {
            writeln!(input, "{i}").unwrap();
        }
        let expected_sum = count * (count - 1) / 2;

        let result = run_within(Duration::from_secs(90), "running a solution with large input and output", move || {
            let tempdir = TempDir::new().unwrap();
            let mut runner = CppRunner::new(tempdir.path()).unwrap();
            let program_handle = runner.add_program(program_source).unwrap();
            runner.check_programs(&input, &[program_handle], 10000).unwrap().remove(0)
        });

        assert!(matches!(result, RunResult::Ok(..)), "Expected OK but got {result:?}");
        if let RunResult::Ok(_, output) = result {
            let lines: Vec<&str> = output.lines().collect();
            assert_eq!(lines.len(), count as usize + 1, "output was truncated");
            assert_eq!(lines[count as usize], format!("sum {expected_sum}"));
        }
    }

    /// Solutions routinely keep debug prints on stderr. That must not be mistaken
    /// for the timer's own report.
    #[test]
    fn test_solution_writing_to_stderr_is_still_measured() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path()).unwrap();

        // Includes text that looks like a timing line and an unterminated line.
        let program_source = r#"
        #include <iostream>
        int main() {
            std::cerr << "debug 123\nnot a number\n__EZCP_RESULT__ TLE 999\n";
            std::cerr << "trailing without newline";
            std::cout << "42\n";
            return 0;
        }
        "#;

        let program_handle = runner.add_program(program_source).unwrap();
        let result = &runner.check_programs("", &[program_handle], 1000).unwrap()[0];

        assert!(matches!(result, RunResult::Ok(..)), "Expected OK but got {result:?}");
        if let RunResult::Ok(_, output) = result {
            assert_eq!(output.trim(), "42");
        }

        drop(tempdir);
    }

    fn build_folder_files(build_folder: &std::path::Path) -> Vec<std::path::PathBuf> {
        std::fs::read_dir(build_folder).unwrap().flatten().map(|entry| entry.path()).collect()
    }

    /// A failing task must not strand the ones that are still running, and it must
    /// not leave `run_tasks` waiting for tasks it has decided never to start.
    #[test]
    #[cfg(not(windows))]
    fn test_run_tasks_reports_failure_without_hanging() {
        initialize_logger();

        let result = run_within(Duration::from_secs(60), "running tasks that all fail to start", || {
            let tempdir = TempDir::new().unwrap();
            // Creating the runner builds the timer, which is the only binary in the
            // build folder at this point.
            let mut runner = CppRunner::new(tempdir.path()).unwrap();
            let binaries: Vec<_> = build_folder_files(tempdir.path()).into_iter().filter(|path| path.extension().is_none()).collect();
            assert_eq!(binaries.len(), 1, "the timer should be the only binary in a fresh build folder");
            let timer = binaries[0].clone();

            let program_handle = runner.add_program(HELLO_WORLD_PROGRAM).unwrap();

            // With the timer gone no task can be started at all, so every one of
            // them fails. More tasks than worker threads, so some are still queued
            // when the first failure comes in.
            std::fs::remove_file(&timer).unwrap();

            for _ in 0..20 {
                runner.add_task(program_handle, Arc::from(""), 1000);
            }
            runner.run_tasks(None, false)
        });

        assert!(result.is_err(), "a task that cannot be started must be reported, got {result:?}");
    }

    /// Cleaning the build folder must drop strays but keep everything the runner
    /// still needs, which only holds if all paths are normalised the same way.
    #[test]
    fn test_clean_build_folder_keeps_compiled_programs() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut runner = CppRunner::new(tempdir.path()).unwrap();
        let program_handle = runner.add_program(HELLO_WORLD_PROGRAM).unwrap();

        let stray = tempdir.path().join("stray.txt");
        std::fs::write(&stray, "junk").unwrap();

        runner.add_task(program_handle, Arc::from(""), 1000);
        runner.run_tasks(None, true).unwrap();

        assert!(!stray.exists(), "cleanup should have removed the stray file");

        // Both the solution and the timer must have survived the cleanup.
        runner.clear_tasks();
        let result = &runner.check_programs("", &[program_handle], 1000).unwrap()[0];
        assert!(matches!(result, RunResult::Ok(..)), "Expected OK but got {result:?}");

        drop(tempdir);
    }

    #[test]
    #[cfg(not(windows))]
    fn test_runner_pickup_cache() {
        initialize_logger();

        let tempdir = TempDir::new().unwrap();
        let mut start = Instant::now();

        for it in 0..30 {
            if it == 1 {
                // After the first iteration, we should have a cache
                start = Instant::now();
            }
            let mut runner = CppRunner::new(tempdir.path()).unwrap();

            let program_handle = runner.add_program(HELLO_WORLD_PROGRAM).unwrap();
            let result = &runner.check_programs("", &[program_handle], 1000).unwrap()[0];

            assert!(matches!(result, RunResult::Ok(..)));

            if let RunResult::Ok(_, output) = result {
                assert_eq!(output.trim(), "Hello, World!");
            }
        }

        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 10, "Cache pickup took too long: {elapsed:?}");

        drop(tempdir);
    }
}
