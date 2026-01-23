use std::collections::HashSet;

/// A solution implementation (correct or partial) to be tested.
///
/// A solution is defined by its source code and a set of subtasks it is
/// expected to pass. The system uses this information during test generation
/// to ensure that robust tests are found that correctly distinguish between
/// different solution implementations.
pub struct Solution {
    /// The C++ source code for the solution.
    pub source: String,
    /// Indices of the subtasks this solution is designed to pass.
    pub passes_subtasks: HashSet<usize>,
}

impl Solution {
    /// Creates a new `Solution` instance.
    ///
    /// * `source` - C++ source code.
    /// * `passes_subtasks` - A slice of subtask indices (0-indexed) that the
    ///   solution should successfully solve.
    #[must_use]
    pub fn new(source: String, passes_subtasks: &[usize]) -> Self {
        Self {
            source,
            passes_subtasks: passes_subtasks.iter().copied().collect(),
        }
    }

    /// Returns `true` if this solution is expected to fail on the specified subtask.
    #[must_use]
    pub fn should_fail(&self, subtask: usize) -> bool {
        !self.passes_subtasks.contains(&subtask)
    }
}
