#[cfg(test)]
mod test_generator_tests {
    use crate::rng::Rng;
    use crate::test::TestGenerator;

    #[test]
    fn test_generator_new_and_generate() {
        let generator = TestGenerator::new(|_rng| "hello world".to_owned());
        assert_eq!(generator.generate(1), "hello world");
    }

    #[test]
    fn test_generator_multiple_calls() {
        let generator = TestGenerator::new(|_rng| "42\n".to_owned());
        for seed in 0..20 {
            assert_eq!(generator.generate(seed), "42\n");
        }
    }

    #[test]
    fn test_generator_captures_state() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        let generator = TestGenerator::new(move |_rng| {
            let n = counter_clone.fetch_add(1, Ordering::SeqCst);
            format!("{n}")
        });

        assert_eq!(generator.generate(0), "0");
        assert_eq!(generator.generate(0), "1");
        assert_eq!(generator.generate(0), "2");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_generator_multiline_output() {
        let generator = TestGenerator::new(|_rng| "3\n1 2 3\n".to_owned());
        assert_eq!(generator.generate(7), "3\n1 2 3\n");
    }

    /// The whole point of handing a generator a seed: the same one has to give
    /// the same test back, and a different one has to give a different test.
    #[test]
    fn a_seed_decides_what_a_generator_produces() {
        let generator = TestGenerator::new(|rng: &mut Rng| rng.random_range(0..1_000_000).to_string());

        assert_eq!(generator.generate(123), generator.generate(123));
        assert_ne!(generator.generate(123), generator.generate(124));
    }

    /// A generator must not be able to tell how many tests came before it, or a
    /// test could not be rebuilt on its own from its seed.
    #[test]
    fn a_generator_does_not_depend_on_earlier_calls() {
        let generator = TestGenerator::new(|rng: &mut Rng| rng.random_range(0..1_000_000).to_string());

        let first = generator.generate(99);
        for seed in 0..50 {
            let _ = generator.generate(seed);
        }
        assert_eq!(generator.generate(99), first);
    }
}
