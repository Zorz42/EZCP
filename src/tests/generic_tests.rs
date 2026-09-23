#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod generic_tests {
    use crate::to_output::ToOutput;
    use crate::{Error, Mode, Subtask, Task};
    use log::LevelFilter;
    use tempfile::TempDir;

    /// A task in a fresh directory, which is deleted with the returned guard.
    pub fn test_task<T: ToOutput>() -> (TempDir, Task<T>) {
        let dir = TempDir::new().unwrap();
        let task = Task::new("Test task", dir.path()).with_debug_level(LevelFilter::Trace);
        (dir, task)
    }

    const RETURN_ZERO: &str = "int main() { return 0; }";
    const PRINT_ONE: &str = "#include <iostream>\nint main() { std::cout << \"1\\n\"; }";

    /// Three subtasks with eight tests between them.
    fn with_three_subtasks(task: Task<String>) -> Task<String> {
        let subtask = |count: usize| (1..=count).fold(Subtask::new(0, ""), |subtask, i| subtask.with_test(1, move |_rng| format!("{i}\n")));
        task.with_solution_source(PRINT_ONE).with_subtask(subtask(3)).with_subtask(subtask(3)).with_subtask(subtask(2))
    }

    #[test]
    fn a_task_without_subtasks_succeeds() {
        let (_dir, task) = test_task::<String>();
        task.with_solution_source(RETURN_ZERO).run_mode(Mode::Files).unwrap();
    }

    #[test]
    fn create_with_empty_subtasks() {
        let (_dir, task) = test_task::<String>();
        task.with_solution_source(PRINT_ONE)
            .with_subtask(Subtask::new(0, ""))
            .with_subtask(Subtask::new(0, ""))
            .run_mode(Mode::Files)
            .unwrap();
    }

    #[test]
    fn create_with_tests() {
        let (_dir, task) = test_task();
        with_three_subtasks(task).run_mode(Mode::Files).unwrap();
    }

    #[test]
    fn create_with_custom_names() {
        let schemes: [fn(i32, i32, i32) -> String; 3] = [
            |test_id, subtask_id, id_in_subtask| format!("{subtask_id}_{id_in_subtask}_{test_id}"),
            |_test_id, subtask_id, id_in_subtask| format!("{subtask_id}_{id_in_subtask}"),
            |test_id, _subtask_id, _id_in_subtask| format!("{test_id}"),
        ];
        for name in schemes {
            let (dir, task) = test_task();
            with_three_subtasks(task)
                .with_get_input_file_name(move |a, b, c| format!("in_{}.txt", name(a, b, c)))
                .with_get_output_file_name(move |a, b, c| format!("out_{}.txt", name(a, b, c)))
                .run_mode(Mode::Files)
                .unwrap();
            assert_eq!(std::fs::read_dir(dir.path().join("tests")).unwrap().count(), 16);
        }
    }

    #[test]
    fn test_fails_without_solution() {
        let (_dir, task) = test_task::<String>();
        assert!(matches!(task.run_mode(Mode::Files), Err(Error::MissingSolution)));
    }

    #[test]
    fn test_times_out() {
        let (_dir, task) = test_task();
        let fibonacci = "#include <iostream>\nint fib(int a) { return a <= 2 ? 1 : fib(a - 1) + fib(a - 2); }\nint main() { std::cout << fib(100); }";
        let task = task
            .with_time_limit(100)
            .with_solution_source(fibonacci)
            .with_subtask(Subtask::new(0, "").with_test(1, |_rng| "1\n".to_owned()));
        assert!(matches!(task.run_mode(Mode::Files), Err(Error::SolutionTimedOut { .. })));
    }

    #[test]
    fn test_compile_error() {
        let (_dir, task) = test_task();
        let task = task
            .with_solution_source("int main() { this is a compile error }")
            .with_subtask(Subtask::new(0, "").with_test(1, |_rng| "1\n".to_owned()));
        assert!(matches!(task.run_mode(Mode::Files), Err(Error::CompilerError { .. })));
    }

    #[test]
    fn test_stale_build_artifacts_are_removed_on_the_next_run() {
        let tempdir = TempDir::new().unwrap();
        let sources_in_build_folder = || {
            let mut sources = std::fs::read_dir(tempdir.path().join("build"))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "cpp"))
                .collect::<Vec<_>>();
            sources.sort();
            sources
        };
        let run_with = |solution: &str| {
            Task::new("stale artifacts", tempdir.path())
                .with_solution_source(solution)
                .with_subtask(Subtask::new(0, "").with_test(1, |_rng| "1\n".to_owned()))
                .run_mode(Mode::Files)
                .unwrap();
        };

        run_with(RETURN_ZERO);
        let after_first_run = sources_in_build_folder();
        assert_eq!(after_first_run.len(), 2, "expected the timer and one solution, got {after_first_run:?}");

        run_with("int main() { return 1 - 1; }");
        let after_second_run = sources_in_build_folder();
        assert_eq!(after_second_run.len(), 2, "the first solution should not have been kept, got {after_second_run:?}");
        assert_ne!(after_first_run, after_second_run, "the edited solution should have replaced the original one");
    }

    #[test]
    fn test_colliding_test_file_names_are_reported() {
        let (_dir, task) = test_task();
        let task = task
            .with_solution_source(RETURN_ZERO)
            .with_get_input_file_name(|_, _, _| "test.in".to_owned())
            .with_get_output_file_name(|_, _, _| "test.out".to_owned())
            .with_subtask(Subtask::new(0, "").with_test(1, |_rng| "1\n".to_owned()).with_test(1, |_rng| "2\n".to_owned()));
        assert!(matches!(task.run_mode(Mode::Files), Err(Error::TestAlreadyExists { .. })));
    }

    #[test]
    fn test_task_large_time_limit_does_not_panic() {
        let (_dir, task) = test_task();
        let _ = task
            .with_time_limit(1_000_000)
            .with_solution_source(RETURN_ZERO)
            .with_subtask(Subtask::new(0, "").with_test(1, |_rng| "\n".to_owned()))
            .run_mode(Mode::Files);
    }
}
