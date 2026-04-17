#[cfg(test)]
mod solution_tests {
    use crate::Solution;

    #[test]
    fn test_solution_new_basic() {
        let sol = Solution::new("sol_basic".to_owned(), "int main(){}".to_owned(), &[0, 1, 2]);
        assert!(sol.passes_subtasks.contains(&0));
        assert!(sol.passes_subtasks.contains(&1));
        assert!(sol.passes_subtasks.contains(&2));
        assert!(!sol.passes_subtasks.contains(&3));
    }

    #[test]
    fn test_solution_should_fail_true() {
        // Solution that only passes subtask 0 should fail subtask 1
        let sol = Solution::new("sol_fail".to_owned(), "src".to_owned(), &[0]);
        assert!(sol.should_fail(1));
        assert!(sol.should_fail(2));
        assert!(sol.should_fail(99));
    }

    #[test]
    fn test_solution_should_fail_false() {
        let sol = Solution::new("sol_pass".to_owned(), "src".to_owned(), &[0, 2]);
        assert!(!sol.should_fail(0));
        assert!(!sol.should_fail(2));
    }

    #[test]
    fn test_solution_should_fail_empty_pass_list() {
        // A solution that passes no subtasks should fail every subtask
        let sol = Solution::new("sol_empty".to_owned(), "src".to_owned(), &[]);
        for i in 0..10 {
            assert!(sol.should_fail(i));
        }
    }

    #[test]
    fn test_solution_passes_all_subtasks() {
        let sol = Solution::new("sol_all".to_owned(), "src".to_owned(), &[0, 1, 2, 3, 4]);
        for i in 0..5 {
            assert!(!sol.should_fail(i));
        }
        // Still fails subtasks outside the list
        assert!(sol.should_fail(5));
    }

    #[test]
    fn test_solution_source_stored() {
        let src = "int main() { return 42; }".to_owned();
        let sol = Solution::new("sol_source".to_owned(), src.clone(), &[0]);
        assert_eq!(sol.source, src);
    }

    #[test]
    fn test_solution_duplicate_subtask_indices() {
        // Duplicate indices should be deduplicated via HashSet
        let sol = Solution::new("sol_dup".to_owned(), "src".to_owned(), &[1, 1, 1]);
        assert!(!sol.should_fail(1));
        assert_eq!(sol.passes_subtasks.len(), 1);
    }
}
