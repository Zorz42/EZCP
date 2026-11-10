#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod array_tests {
    use crate::rng::Rng;
    use crate::tests::generic_tests::generic_tests::test_task;
    use crate::{Counted, Mode, Subtask, ToOutput, array_generator, array_generator_custom, array_to_string};

    #[test]
    fn test_array_generator_in_a_task() {
        let solution = "#include <iostream>\nint main() { int n; std::cin >> n; for (int i = 0; i < n; i++) { int a; std::cin >> a; } std::cout << \"1\\n\"; }";
        let (_dir, task) = test_task();
        task.with_solution_source(solution)
            .with_subtask(
                Subtask::new(0, "")
                    .with_test(5, array_generator(1..=100, 1..=100))
                    .with_test(5, array_generator(1..=100, 1..=1))
                    .with_test(5, array_generator(100..=100, 1..=100))
                    .with_test(1, array_generator(100..=100, 1..=1)),
            )
            .with_subtask(Subtask::new(0, "").with_test(5, array_generator(42..=42, 1..=100)).with_test(5, array_generator(42..=42, 100..=100)))
            .with_subtask(Subtask::new(0, "").with_test(5, array_generator(1..=100, 47..=47)).with_test(5, array_generator(1..=1, 47..=47)))
            .run_mode(Mode::Files)
            .unwrap();
    }

    #[test]
    fn test_array_to_string_format() {
        assert_eq!(array_to_string(&[1, 2, 3], true), "3\n1 2 3\n");
        assert_eq!(array_to_string(&[1, 2, 3], false), "1 2 3\n");
        assert_eq!(array_to_string(&[7], true), "1\n7\n");
        assert_eq!(array_to_string::<i32>(&[], true), "0\n\n");
        assert_eq!(array_to_string::<i32>(&[], false), "\n");
        assert_eq!(array_to_string(&["a", "b"], false), "a b\n");
    }

    #[test]
    fn test_array_generators_of_other_types() {
        let mut rng = Rng::from_seed(3);
        let Counted(large) = array_generator(4..=4, 1_000_000_000_000..=2_000_000_000_000_i64)(&mut rng);
        assert!(large.len() == 4 && large.iter().all(|value| (1_000_000_000_000..=2_000_000_000_000).contains(value)), "{large:?}");

        let Counted(letters) = array_generator(3..=9, 'a'..='c')(&mut rng);
        assert!((3..=9).contains(&letters.len()) && letters.iter().all(|letter| ('a'..='c').contains(letter)), "{letters:?}");

        let Counted(pairs) = array_generator_custom(2..3, |rng| rng.random_interval(1..=9))(&mut rng);
        assert!(pairs.len() == 2 && pairs.iter().all(|&(left, right)| left <= right), "{pairs:?}");
        assert_eq!(array_generator_custom(1..=1, |_rng| (4, 5))(&mut rng).to_output(), "1\n4 5\n");
    }

    #[test]
    fn test_array_generator_length_and_values() {
        for seed in 0..50 {
            let output = array_generator(5..=8, 10..=20)(&mut Rng::from_seed(seed)).to_output();
            let mut lines = output.lines();
            let count: usize = lines.next().unwrap().parse().unwrap();
            let values: Vec<i32> = lines.next().unwrap().split_whitespace().map(|value| value.parse().unwrap()).collect();
            assert!((5..=8).contains(&count), "length {count} out of range");
            assert_eq!(values.len(), count);
            assert!(values.iter().all(|value| (10..=20).contains(value)), "values out of range: {values:?}");
        }
    }
}
