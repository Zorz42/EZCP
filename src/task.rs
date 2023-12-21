use std::path::PathBuf;
use crate::subtask::Subtask;

/// This struct contains all the information about a competitive programming task
pub struct Task {
    pub name: String,
    pub path: PathBuf,
    pub tests_path: PathBuf,
    pub subtasks: Vec<Subtask>,
}