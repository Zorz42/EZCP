#[cfg(test)]
mod test_generator_tests {
    use crate::test::TestGenerator;

    #[test]
    fn test_generator_new_and_generate() {
        let generator = TestGenerator::new(|| "hello world".to_owned());
        assert_eq!(generator.generate(), "hello world");
    }

    #[test]
    fn test_generator_multiple_calls() {
        let generator = TestGenerator::new(|| "42\n".to_owned());
        for _ in 0..20 {
            assert_eq!(generator.generate(), "42\n");
        }
    }

    #[test]
    fn test_generator_captures_state() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        let generator = TestGenerator::new(move || {
            let n = counter_clone.fetch_add(1, Ordering::SeqCst);
            format!("{n}")
        });

        assert_eq!(generator.generate(), "0");
        assert_eq!(generator.generate(), "1");
        assert_eq!(generator.generate(), "2");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_generator_multiline_output() {
        let generator = TestGenerator::new(|| "3\n1 2 3\n".to_owned());
        assert_eq!(generator.generate(), "3\n1 2 3\n");
    }
}
