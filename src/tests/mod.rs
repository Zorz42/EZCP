#![allow(clippy::expect_used)]

mod archiver_tests;
mod array_tests;
mod checker_tests;
mod cpp_runner_tests;
mod exec_runner_tests;
mod feature_tests;
mod frankenstein_tests;
mod gcc_tests;
mod generic_tests;
mod golden_tests;
mod graph_tests;
mod mode_tests;
mod partial_solution_tests;
mod rng_tests;
mod seed_mode_tests;
mod stub_tests;
mod subtask_tests;
mod to_output_tests;
mod trim_whitespace_tests;

use crate::{Task, ToOutput};
use log::LevelFilter;
use std::path::Path;
use tempfile::TempDir;

pub const DOUBLE: &str = "#include <iostream>\nint main() { long long n; std::cin >> n; std::cout << 2 * n << '\\n'; }";

/// [`DOUBLE`], but one more where `condition` holds.
pub fn wrong_when(condition: &str) -> String {
    format!("#include <iostream>\nint main() {{ long long n; std::cin >> n; std::cout << (({condition}) ? 2 * n + 1 : 2 * n) << '\\n'; }}")
}

/// In a fresh directory, deleted with the guard.
pub fn test_task<T: ToOutput>() -> (TempDir, Task<T>) {
    let dir = TempDir::new().unwrap();
    let task = Task::new("Test task", dir.path()).with_debug_level(LevelFilter::Trace);
    (dir, task)
}

/// `(name, contents)` of every test file, sorted.
pub fn test_files(path: &Path) -> Vec<(String, String)> {
    let mut files: Vec<_> = std::fs::read_dir(path.join("tests"))
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name().to_string_lossy().into_owned(), std::fs::read_to_string(entry.path()).unwrap())
        })
        .collect();
    files.sort();
    files
}
