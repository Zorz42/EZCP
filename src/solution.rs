use std::collections::HashSet;

/// A partial solution and the subtasks it is expected to pass.
pub struct Solution {
    /// Identifies the solution in errors and results.
    pub name: String,
    /// C++ source code.
    pub source: String,
    /// 0-based indices of the subtasks it is expected to pass.
    pub passes_subtasks: HashSet<usize>,
}

impl Solution {
    /// Creates a solution expected to pass `passes_subtasks` (0-based).
    #[must_use]
    pub fn new(name: String, source: String, passes_subtasks: &[usize]) -> Self {
        Self {
            name,
            source,
            passes_subtasks: passes_subtasks.iter().copied().collect(),
        }
    }

    /// Whether the solution is expected to fail `subtask`.
    #[must_use]
    pub fn should_fail(&self, subtask: usize) -> bool {
        !self.passes_subtasks.contains(&subtask)
    }
}
