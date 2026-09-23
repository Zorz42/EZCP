#[cfg(test)]
#[allow(clippy::expect_used)]
mod subtask_tests {
    use crate::Subtask;
    use crate::rng::Rng;
    use std::collections::HashSet;

    fn three_generators() -> Subtask<String> {
        Subtask::new(0, "t")
            .with_test(1, |_rng| "A".to_owned())
            .with_test(2, |_rng| "B".to_owned())
            .with_test(5, |_rng| "C".to_owned())
    }

    #[test]
    fn test_subtask_new() {
        let st = Subtask::<String>::new(0, "my subtask");
        assert_eq!(st.name, "my subtask");
        assert_eq!(st.get_num_generators(), 0);
        assert!(st.initial_counts.is_empty());
        assert!(st.pick_generator(&mut Rng::from_seed(0)).is_none());
    }

    #[test]
    fn test_subtask_with_test_adds_generators() {
        let st = three_generators();
        assert_eq!(st.get_num_generators(), 3);
        assert_eq!(st.initial_counts, vec![1, 2, 5]);
    }

    #[test]
    fn test_pick_generator_uses_every_generator() {
        let st = three_generators();
        let mut rng = Rng::from_seed(2);
        let seen: HashSet<_> = (0..200).map(|_| st.generate_test(st.pick_generator(&mut rng).expect("should return Some"), 0)).collect();
        assert_eq!(seen.len(), 3, "expected all generators to be used");
    }

    #[test]
    fn picking_a_generator_is_reproducible() {
        let st = three_generators();
        let picks = |seed| {
            let mut rng = Rng::from_seed(seed);
            (0..20).filter_map(|_| st.pick_generator(&mut rng)).collect::<Vec<_>>()
        };
        assert_eq!(picks(5), picks(5));
        assert_ne!(picks(5), picks(6));
    }

    #[test]
    fn a_seed_decides_the_test() {
        let st = Subtask::new(0, "t").with_test(1, |rng| rng.random_range(0..1_000_000).to_string());
        let first = st.generate_test(0, 99);
        for seed in 0..50 {
            let _ = st.generate_test(0, seed);
        }
        assert_eq!(st.generate_test(0, 99), first);
        assert_ne!(st.generate_test(0, 100), first);
    }

    #[test]
    #[should_panic(expected = "too large")]
    fn a_checker_rejects_a_test_outside_the_constraints() {
        let st = Subtask::new(0, "t").with_test(1, |_rng| "1000000\n".to_owned()).with_checker(|test: &String| {
            assert!(test.trim().parse::<i64>().unwrap_or(0) <= 10, "too large");
        });
        let _ = st.generate_test(0, 0);
    }
}
