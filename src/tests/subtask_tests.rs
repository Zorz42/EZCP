#[cfg(test)]
#[allow(clippy::expect_used)]
mod subtask_tests {
    use crate::Subtask;
    use crate::rng::Rng;

    #[test]
    fn test_subtask_new() {
        let st = Subtask::<String>::new(0, "my subtask");
        assert_eq!(st.name, "my subtask");
        assert_eq!(st.get_num_generators(), 0);
        assert!(st.initial_counts.is_empty());
    }

    #[test]
    fn test_subtask_default_name_empty() {
        let st = Subtask::<String>::new(0, "");
        assert_eq!(st.name, "");
    }

    #[test]
    fn test_subtask_with_test_adds_generator() {
        let st = Subtask::new(0, "t").with_test(3, |_rng| "hello".to_owned());
        assert_eq!(st.get_num_generators(), 1);
        assert_eq!(st.initial_counts.len(), 1);
        assert_eq!(st.initial_counts[0], 3);
    }

    #[test]
    fn test_subtask_with_test_multiple_generators() {
        let st = Subtask::new(0, "t")
            .with_test(1, |_rng| "a".to_owned())
            .with_test(2, |_rng| "b".to_owned())
            .with_test(5, |_rng| "c".to_owned());
        assert_eq!(st.get_num_generators(), 3);
        assert_eq!(st.initial_counts, vec![1, 2, 5]);
    }

    #[test]
    fn test_pick_generator_no_generators_returns_none() {
        let st = Subtask::<String>::new(0, "empty");
        assert!(st.pick_generator(&mut Rng::from_seed(0)).is_none());
    }

    #[test]
    fn test_pick_generator_single_generator() {
        let st = Subtask::new(0, "t").with_test(1, |_rng| "42\n".to_owned());
        let mut rng = Rng::from_seed(1);
        for _ in 0..10 {
            let gen_idx = st.pick_generator(&mut rng).expect("should return Some");
            assert_eq!(gen_idx, 0);
            assert_eq!(st.generate_test(gen_idx, 0), "42\n");
        }
    }

    #[test]
    fn test_pick_generator_multiple_generators_returns_one_of_values() {
        let st = Subtask::new(0, "t")
            .with_test(1, |_rng| "A".to_owned())
            .with_test(1, |_rng| "B".to_owned())
            .with_test(1, |_rng| "C".to_owned());

        let mut rng = Rng::from_seed(2);
        let mut seen = std::collections::HashSet::new();
        // Run enough times to likely hit all three generators
        for _ in 0..200 {
            let gen_idx = st.pick_generator(&mut rng).expect("should return Some");
            let val = st.generate_test(gen_idx, 0);
            assert!(["A", "B", "C"].contains(&val.as_str()), "unexpected: {val}");
            seen.insert(val);
        }
        // With 200 trials, all 3 should be seen (probability of missing one is ~(2/3)^200 ≈ 0)
        assert_eq!(seen.len(), 3, "expected all generators to be used");
    }

    #[test]
    fn test_generate_test_preserves_generator_output() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        let st = Subtask::new(0, "t").with_test(1, move |_rng| counter_clone.fetch_add(1, Ordering::SeqCst).to_string());

        let _ = st.generate_test(0, 0);
        let _ = st.generate_test(0, 1);
        let _ = st.generate_test(0, 2);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    /// The generator that gets picked has to come from the run's own generator,
    /// so that a seed decides the whole shape of a run and not just the tests.
    #[test]
    fn picking_a_generator_is_reproducible() {
        let subtask = || {
            Subtask::new(0, "t")
                .with_test(1, |_rng| "A".to_owned())
                .with_test(1, |_rng| "B".to_owned())
                .with_test(1, |_rng| "C".to_owned())
        };

        let picks = |seed| {
            let st = subtask();
            let mut rng = Rng::from_seed(seed);
            (0..20).filter_map(|_| st.pick_generator(&mut rng)).collect::<Vec<_>>()
        };

        assert_eq!(picks(5), picks(5));
        assert_ne!(picks(5), picks(6));
    }

    /// A subtask's checker runs on generated tests, and a test outside the
    /// subtask's constraints has to be caught rather than written out.
    #[test]
    #[should_panic(expected = "too large")]
    fn a_checker_rejects_a_test_outside_the_constraints() {
        let st = Subtask::new(0, "t").with_test(1, |_rng| "1000000\n".to_owned()).with_checker(|test: &String| {
            assert!(test.trim().parse::<i64>().unwrap_or(0) <= 10, "too large");
        });
        let _ = st.generate_test(0, 0);
    }
}
