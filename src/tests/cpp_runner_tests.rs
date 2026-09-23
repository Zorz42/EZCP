#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod cpp_runner_tests {
    use crate::Error::{CompilerError, TimerFailed};
    use crate::runner::cpp_runner::CppRunner;
    use crate::runner::exec_runner::RunResult;
    use crate::tests::test_shared::initialize_logger;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    const HELLO_WORLD: &str = "#include <iostream>\nint main() { std::cout << \"Hello, World!\" << std::endl; }";

    /// Fails the test instead of hanging if `body` does not finish in time.
    #[allow(clippy::panic)]
    fn run_within<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(timeout: Duration, what: &str, body: F) -> T {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(body());
        });
        receiver.recv_timeout(timeout).unwrap_or_else(|_| panic!("{what} did not finish within {timeout:?}"))
    }

    /// A runner in a fresh build folder, which is deleted with the returned guard.
    fn runner() -> (TempDir, CppRunner) {
        initialize_logger();
        let dir = TempDir::new().unwrap();
        let runner = CppRunner::new(dir.path()).unwrap();
        (dir, runner)
    }

    /// Compiles `source` and runs it once on `input`.
    fn run(source: &str, input: &str, time_limit: i32) -> RunResult {
        let (_dir, mut runner) = runner();
        let program = runner.add_program(source).unwrap();
        runner.check_programs(input, &[program], time_limit).unwrap().remove(0)
    }

    fn assert_output(result: &RunResult, expected: &str) {
        assert!(matches!(result, RunResult::Ok(_, output) if output.trim() == expected), "expected {expected:?}, got {result:?}");
    }

    fn build_folder_files(build_folder: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(build_folder).unwrap().map(|entry| entry.unwrap().path()).collect()
    }

    #[test]
    fn test_runner_run_program() {
        assert_output(&run(HELLO_WORLD, "", 1000), "Hello, World!");
    }

    #[test]
    fn test_runner_add_faulty_program() {
        let (_dir, mut runner) = runner();
        assert!(matches!(runner.add_program("int main() { compile error here }"), Err(CompilerError { .. })));
    }

    #[test]
    fn test_runner_run_programs() {
        let (_dir, mut runner) = runner();
        let programs: Vec<_> = (0..5)
            .map(|id| {
                runner
                    .add_program(&format!("#include <iostream>\nint main() {{ int n; std::cin >> n; std::cout << {id} << ' ' << n; }}"))
                    .unwrap()
            })
            .collect();

        for i in 0..20 {
            for (j, result) in runner.check_programs(&format!("{i}\n"), &programs, 1000).unwrap().iter().enumerate() {
                assert_output(result, &format!("{j} {i}"));
            }
        }
    }

    #[test]
    fn test_same_program_100_times() {
        let (_dir, mut runner) = runner();
        let time = Instant::now();
        let handles: Vec<_> = (0..100).map(|_| runner.add_program(HELLO_WORLD).unwrap()).collect();
        assert!(handles.windows(2).all(|pair| pair[0] == pair[1]));
        assert!(time.elapsed().as_secs() < 10, "Adding the same program 100 times took too long");
    }

    #[test]
    fn test_runner_program_tle() {
        assert_eq!(run("int main() { while (true) {} }", "", 1000), RunResult::TimedOut);
    }

    #[test]
    fn test_program_over_a_sub_second_limit_is_a_timeout() {
        let (_dir, mut runner) = runner();
        // 400 ms: over the limit, but under the one second the timer rounds up to.
        let program = runner
            .add_program("#include <chrono>\nint main() { auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(400); while (std::chrono::steady_clock::now() < deadline) {} }")
            .unwrap();

        assert_eq!(runner.check_programs("", &[program], 100).unwrap()[0], RunResult::TimedOut);
        assert!(matches!(runner.check_programs("", &[program], 5000).unwrap()[0], RunResult::Ok(..)));
    }

    #[test]
    fn test_deep_recursion_has_a_large_stack() {
        // About 100 MB of stack, far beyond the usual 8 MB default.
        let source = "#include <iostream>
            void recurse(int depth) { volatile char frame[1024]; for (int i = 0; i < 1024; ++i) frame[i] = (char)i; if (depth > 0) recurse(depth - 1); if (frame[0] != 0) std::cout << 'x'; }
            int main() { recurse(100000); std::cout << \"Success\"; }";
        assert_output(&run(source, "", 2000), "Success");
    }

    #[test]
    fn test_runner_program_crash() {
        assert_eq!(run("int main() { volatile int* p = nullptr; *p = 1; }", "", 1000), RunResult::Crashed);
    }

    /// Must not leave EZCP blocked writing to a full input pipe.
    #[test]
    fn test_solution_that_ignores_most_of_its_input() {
        // Larger than any platform's pipe buffer.
        let input = format!("7\n{}", (0..500_000).map(|i| i.to_string()).collect::<Vec<_>>().join(" "));
        let result = run_within(Duration::from_secs(90), "running a solution that ignores its input", move || {
            run("#include <iostream>\nint main() { long long n; std::cin >> n; std::cout << n; }", &input, 5000)
        });
        assert_output(&result, "7");
    }

    #[test]
    fn test_large_input_and_large_output() {
        let count = 200_000_usize;
        let input = (0..count).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        let source = "#include <iostream>
            int main() { std::ios_base::sync_with_stdio(false); long long sum = 0, x; while (std::cin >> x) { sum += x; std::cout << x << '\\n'; } std::cout << \"sum \" << sum; }";

        let result = run_within(Duration::from_secs(90), "running a solution with large input and output", move || run(source, &input, 10000));

        let RunResult::Ok(_, output) = result else { unreachable!("expected OK, got {result:?}") };
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), count + 1, "output was truncated");
        assert_eq!(lines[count], format!("sum {}", count * (count - 1) / 2));
    }

    #[test]
    fn test_solution_writing_to_stderr_is_still_measured() {
        // Includes a fake marker and an unterminated line.
        let source = "#include <iostream>
            int main() { std::cerr << \"debug 123\\nnot a number\\n__EZCP_RESULT__ TLE 999\\ntrailing without newline\"; std::cout << 42; }";
        assert_output(&run(source, "", 1000), "42");
    }

    #[test]
    fn test_solution_writing_a_lot_to_stderr_is_still_measured() {
        // 16 MB, far more than the tail that is kept.
        let source = "#include <iostream>\n#include <string>
            int main() { std::string line(1 << 12, 'd'); for (int i = 0; i < 4096; i++) std::cerr << line << '\\n'; std::cout << 42; }";
        assert_output(&run(source, "", 5000), "42");
    }

    #[test]
    fn test_runaway_output_is_stopped() {
        // Large writes, so the limit is reached quickly even on a slow machine.
        let source = "#include <cstdio>\n#include <string>
            int main() { std::string chunk(1 << 20, 'x'); for (;;) fwrite(chunk.data(), 1, chunk.size(), stdout); }";
        assert_eq!(
            run_within(Duration::from_secs(60), "a solution that never stops printing", move || run(source, "", 5000)),
            RunResult::Crashed
        );
    }

    /// As when a judge starts many `--serve` processes on a fresh build folder.
    #[test]
    fn test_runners_sharing_a_build_folder() {
        initialize_logger();
        let tempdir = TempDir::new().unwrap();
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let build_folder = tempdir.path().to_path_buf();
                std::thread::spawn(move || -> crate::Result<RunResult> {
                    let mut runner = CppRunner::new(&build_folder)?;
                    let program = runner.add_program(HELLO_WORLD)?;
                    Ok(runner.check_programs("", &[program], 5000)?.remove(0))
                })
            })
            .collect();

        for worker in workers {
            assert_output(&worker.join().unwrap().unwrap(), "Hello, World!");
        }
        // The timer and the program, each a source and a binary; no scratch files.
        assert_eq!(build_folder_files(tempdir.path()).len(), 4, "{:?}", build_folder_files(tempdir.path()));
    }

    /// A crash would blame the solution, and count as a failure of a partial
    /// solution that never ran.
    #[test]
    fn test_missing_binary_is_not_reported_as_a_crash() {
        let (dir, mut runner) = runner();
        let binaries = || {
            build_folder_files(dir.path())
                .into_iter()
                .filter(|path| path.extension().is_none_or(|extension| extension != "cpp"))
                .collect::<Vec<_>>()
        };

        let timer_only = binaries();
        let program = runner.add_program(HELLO_WORLD).unwrap();
        std::fs::remove_file(binaries().into_iter().find(|path| !timer_only.contains(path)).unwrap()).unwrap();

        let result = runner.check_programs("", &[program], 1000);
        assert!(matches!(result, Err(TimerFailed { .. })), "got {result:?}");
    }

    #[test]
    #[cfg(not(windows))]
    fn test_run_tasks_reports_failure_without_hanging() {
        let result = run_within(Duration::from_secs(60), "running tasks that all fail to start", || {
            let (dir, mut runner) = runner();
            // The timer is the only binary so far.
            let timer: Vec<_> = build_folder_files(dir.path()).into_iter().filter(|path| path.extension().is_none()).collect();
            assert_eq!(timer.len(), 1, "the timer should be the only binary in a fresh build folder");
            let program = runner.add_program(HELLO_WORLD).unwrap();

            // Every task now fails. More tasks than workers, so some are still queued.
            std::fs::remove_file(&timer[0]).unwrap();
            for _ in 0..20 {
                runner.add_task(program, "".into(), 1000);
            }
            runner.run_tasks(None)
        });
        assert!(result.is_err(), "a task that cannot be started must be reported, got {result:?}");
    }

    #[test]
    fn test_clean_build_folder_keeps_compiled_programs() {
        let (dir, mut runner) = runner();
        let program = runner.add_program(HELLO_WORLD).unwrap();
        let stray = dir.path().join("stray.txt");
        std::fs::write(&stray, "junk").unwrap();

        runner.clean_build_folder().unwrap();

        assert!(!stray.exists(), "cleanup should have removed the stray file");
        assert_output(&runner.check_programs("", &[program], 1000).unwrap()[0], "Hello, World!");
    }

    #[test]
    #[cfg(not(windows))]
    fn test_runner_pickup_cache() {
        initialize_logger();
        let tempdir = TempDir::new().unwrap();
        let run_once = || {
            let mut runner = CppRunner::new(tempdir.path()).unwrap();
            let program = runner.add_program(HELLO_WORLD).unwrap();
            assert_output(&runner.check_programs("", &[program], 1000).unwrap()[0], "Hello, World!");
        };

        run_once();
        let start = Instant::now();
        for _ in 0..29 {
            run_once();
        }
        assert!(start.elapsed().as_secs() < 10, "Cache pickup took too long: {:?}", start.elapsed());
    }
}
