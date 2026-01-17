use std::collections::HashSet;

/// A solution (main or partial) that will be tested against generated tests.
///
/// Each solution specifies which subtasks it is expected to pass.
/// During test generation, the system will generate tests until each
/// solution that is not expected to pass a subtask has failed on at least
/// `min_failures_per_solution` tests for that subtask.
pub struct Solution {
    /// The C++ source code for the solution
    pub source: String,
    /// Set of subtask indices this solution is expected to pass
    pub passes_subtasks: HashSet<usize>,
}

impl Solution {
    /// Create a new solution with the given source code and expected passing subtasks.
    #[must_use]
    pub fn new(source: String, passes_subtasks: &[usize]) -> Self {
        Self {
            source,
            passes_subtasks: passes_subtasks.iter().copied().collect(),
        }
    }

    /// Check if this solution should be expected to fail on a given subtask.
    #[must_use]
    pub fn should_fail(&self, subtask: usize) -> bool {
        !self.passes_subtasks.contains(&subtask)
    }
}
