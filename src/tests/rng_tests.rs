#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod rng_tests {
    use crate::rng::Rng;
    use std::collections::HashSet;

    #[test]
    fn the_same_seed_gives_the_same_stream() {
        let mut a = Rng::from_seed(12345);
        let mut b = Rng::from_seed(12345);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_give_different_streams() {
        let mut seen = HashSet::new();
        for seed in 0..64 {
            assert!(seen.insert(Rng::from_seed(seed).next_u64()), "seed {seed} repeated an earlier stream");
        }
    }

    /// Published seeds depend on this exact stream: it must never change.
    #[test]
    fn the_stream_is_frozen() {
        let mut rng = Rng::from_seed(0);
        let got = [rng.next_u64(), rng.next_u64(), rng.next_u64(), rng.next_u64()];
        assert_eq!(
            got,
            [5_987_356_902_031_041_503, 7_051_070_477_665_621_255, 6_633_766_593_972_829_180, 211_316_841_551_650_330],
            "the random stream changed, which silently changes every task's tests"
        );

        let mut seeded = Rng::from_seed(42);
        assert_eq!(seeded.random_range(0..1_000_000), 814_305);
        assert_eq!(seeded.random_range(-50..=50), -18);
    }

    #[test]
    fn ranges_stay_inside_their_bounds() {
        let mut rng = Rng::from_seed(7);
        for _ in 0..10_000 {
            let value = rng.random_range(10..20);
            assert!((10..20).contains(&value), "{value} is outside 10..20");

            let value = rng.random_range(10..=20);
            assert!((10..=20).contains(&value), "{value} is outside 10..=20");

            let value: i32 = rng.random_range(-5..=5);
            assert!((-5..=5).contains(&value), "{value} is outside -5..=5");
        }
        assert_eq!(rng.random_range(4..=4), 4);
        assert_eq!(rng.random_range(4..5), 4);
    }

    #[test]
    fn full_width_ranges_do_not_overflow() {
        let mut rng = Rng::from_seed(2);
        for _ in 0..1000 {
            let _: u64 = rng.random_range(..);
            let _: i64 = rng.random_range(..);
            let _: i32 = rng.random_range(i32::MIN..=i32::MAX);
            let _: u8 = rng.random_range(..=u8::MAX);
        }
    }

    #[test]
    #[should_panic(expected = "empty range")]
    fn an_empty_range_panics() {
        Rng::from_seed(0).random_range(5..5);
    }

    #[test]
    #[should_panic(expected = "empty range")]
    #[allow(clippy::reversed_empty_ranges)]
    fn a_backwards_range_panics() {
        Rng::from_seed(0).random_range(5..=4);
    }

    #[test]
    fn every_value_of_a_small_range_shows_up() {
        let mut rng = Rng::from_seed(3);
        let mut counts = [0_u32; 6];
        for _ in 0..60_000 {
            counts[rng.random_range(0..6_usize)] += 1;
        }
        for (side, count) in counts.iter().enumerate() {
            assert!((9_000..11_000).contains(count), "side {side} came up {count} times in 60000 rolls");
        }
    }

    #[test]
    fn random_bool_respects_its_probability() {
        let mut rng = Rng::from_seed(4);
        assert!(!rng.random_bool(0.0));
        assert!(rng.random_bool(1.0));

        let trues = (0..10_000).filter(|_| rng.random_bool(0.25)).count();
        assert!((2_200..2_800).contains(&trues), "p = 0.25 came up {trues} times in 10000 draws");
    }

    #[test]
    fn shuffle_is_a_reproducible_permutation() {
        let shuffled = |seed, len| {
            let mut values = (0..len).collect::<Vec<_>>();
            Rng::from_seed(seed).shuffle(&mut values);
            values
        };
        let mut sorted = shuffled(5, 100);
        assert_ne!(sorted, (0..100).collect::<Vec<_>>(), "a shuffle of 100 elements left them in order");
        sorted.sort_unstable();
        assert_eq!(sorted, (0..100).collect::<Vec<_>>());

        assert_eq!(shuffled(9, 50), shuffled(9, 50));
        assert_ne!(shuffled(9, 50), shuffled(10, 50));
        assert!(shuffled(6, 0).is_empty());
        assert_eq!(shuffled(6, 1), vec![0]);
    }

    #[test]
    fn choose_returns_an_element() {
        let mut rng = Rng::from_seed(8);
        assert_eq!(rng.choose::<i32>(&[]), None);
        for _ in 0..100 {
            let chosen = *rng.choose(&[1, 2, 3]).unwrap();
            assert!((1..=3).contains(&chosen));
        }
    }

    #[test]
    fn random_f64_stays_in_the_unit_interval() {
        let mut rng = Rng::from_seed(11);
        for _ in 0..10_000 {
            let value = rng.random_f64();
            assert!((0.0..1.0).contains(&value), "{value} is outside [0, 1)");
        }
    }

    #[test]
    fn entropy_seeds_differ_between_generators() {
        let a = Rng::from_entropy().next_u64();
        let b = Rng::from_entropy().next_u64();
        assert_ne!(a, b);
    }
}
