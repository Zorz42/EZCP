#[cfg(test)]
mod subtask_tests {
    use crate::Subtask;

    #[test]
    fn test_subtask_new() {
        let st = Subtask::new("my subtask");
        assert_eq!(st.name, "my subtask");
        assert!(st.generators.is_empty());
        assert!(st.initial_counts.is_empty());
    }

    #[test]
    fn test_subtask_default_name_empty() {
        let st = Subtask::new("");
        assert_eq!(st.name, "");
    }

    #[test]
    fn test_subtask_with_test_adds_generator() {
        let st = Subtask::new("t").with_test(3, || "hello".to_owned());
        assert_eq!(st.generators.len(), 1);
        assert_eq!(st.initial_counts.len(), 1);
        assert_eq!(st.initial_counts[0], 3);
    }

    #[test]
    fn test_subtask_with_test_multiple_generators() {
        let st = Subtask::new("t").with_test(1, || "a".to_owned()).with_test(2, || "b".to_owned()).with_test(5, || "c".to_owned());
        assert_eq!(st.generators.len(), 3);
        assert_eq!(st.initial_counts, vec![1, 2, 5]);
    }

    #[test]
    fn test_generate_random_test_no_generators_returns_none() {
        let st = Subtask::new("empty");
        assert!(st.generate_random_test().is_none());
    }

    #[test]
    fn test_generate_random_test_single_generator() {
        let st = Subtask::new("t").with_test(1, || "42\n".to_owned());
        for _ in 0..10 {
            let result = st.generate_random_test();
            assert_eq!(result.as_deref(), Some("42\n"));
        }
    }

    #[test]
    fn test_generate_random_test_multiple_generators_returns_one_of_values() {
        let st = Subtask::new("t").with_test(1, || "A".to_owned()).with_test(1, || "B".to_owned()).with_test(1, || "C".to_owned());

        let mut seen = std::collections::HashSet::new();
        // Run enough times to likely hit all three generators
        for _ in 0..200 {
            let val = st.generate_random_test().expect("should return Some");
            assert!(["A", "B", "C"].contains(&val.as_str()), "unexpected: {val}");
            seen.insert(val);
        }
        // With 200 trials, all 3 should be seen (probability of missing one is ~(2/3)^200 ≈ 0)
        assert_eq!(seen.len(), 3, "expected all generators to be used");
    }

    #[test]
    fn test_generate_random_test_preserves_generator_output() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        let st = Subtask::new("t").with_test(1, move || counter_clone.fetch_add(1, Ordering::SeqCst).to_string());

        let _ = st.generate_random_test();
        let _ = st.generate_random_test();
        let _ = st.generate_random_test();
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
}
