#[cfg(test)]
mod solution_tests {
    use crate::Solution;

    #[test]
    fn test_solution_new() {
        let solution = Solution::new("name".to_owned(), "int main() { return 42; }".to_owned(), &[0, 2, 2]);
        assert_eq!(solution.name, "name");
        assert_eq!(solution.source, "int main() { return 42; }");
        assert_eq!(solution.passes_subtasks.len(), 2, "duplicates are merged");
    }

    #[test]
    fn test_solution_should_fail() {
        let solution = Solution::new(String::new(), String::new(), &[0, 2]);
        assert_eq!((0..5).map(|subtask| solution.should_fail(subtask)).collect::<Vec<_>>(), [false, true, false, true, true]);
        assert!((0..10).all(|subtask| Solution::new(String::new(), String::new(), &[]).should_fail(subtask)));
    }
}
