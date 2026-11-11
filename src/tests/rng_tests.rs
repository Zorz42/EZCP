use crate::rng::Rng;
use std::collections::{HashMap, HashSet};

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

/// Every seed ever written depends on this stream.
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
    assert_eq!(shuffled(6, 0), Vec::<i32>::new());
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

#[test]
fn random_vec_has_the_length_and_range_asked_for() {
    let mut rng = Rng::from_seed(1);
    let values: Vec<i64> = rng.random_vec(1000, -3..=3);
    assert_eq!(values.len(), 1000);
    assert_eq!(values.iter().copied().collect::<HashSet<_>>().len(), 7);
    assert_eq!(rng.random_vec::<u8, _>(0, ..), Vec::<u8>::new());
    // The same stream as drawing the values one at a time.
    let (mut a, mut b) = (Rng::from_seed(5), Rng::from_seed(5));
    assert_eq!(a.random_vec(20, 0..100), (0..20).map(|_| b.random_range(0..100)).collect::<Vec<_>>());
}

#[test]
fn random_interval_is_ordered() {
    let mut rng = Rng::from_seed(2);
    let intervals: Vec<(i32, i32)> = (0..2000).map(|_| rng.random_interval(1..=10)).collect();
    assert!(intervals.iter().all(|&(left, right)| 1 <= left && left <= right && right <= 10));
    assert!(intervals.iter().any(|&(left, right)| left == right) && intervals.contains(&(1, 10)));
}

#[test]
fn skew_leans_towards_an_end() {
    let mut rng = Rng::from_seed(3);
    let mut average = |skew| (0..4000).map(|_| rng.random_range_skewed(0..1000, skew)).sum::<i32>() / 4000;
    let (low, even, high) = (average(-3), average(0), average(3));
    assert!((150..250).contains(&low), "the smallest of four values averaged {low}");
    assert!((450..550).contains(&even), "one value averaged {even}");
    assert!((750..850).contains(&high), "the largest of four values averaged {high}");
    assert_eq!(rng.random_range_skewed(7..=7, 5), 7);

    let (mut a, mut b) = (Rng::from_seed(5), Rng::from_seed(5));
    assert_eq!(a.random_range_skewed(0..1000, 0), b.random_range(0..1000), "no skew is a plain draw");
}

#[test]
fn shuffled_and_permutation_keep_every_value() {
    let mut rng = Rng::from_seed(4);
    let mut letters = rng.shuffled("hello".chars());
    letters.sort_unstable();
    assert_eq!(letters, ['e', 'h', 'l', 'l', 'o']);

    let permutation = rng.permutation(50);
    assert_ne!(permutation, (1..=50).collect::<Vec<_>>());
    assert_eq!(permutation.iter().copied().collect::<HashSet<_>>(), (1..=50).collect());
    assert_eq!(rng.permutation(0), Vec::<usize>::new());
    assert_eq!(rng.permutation(1), [1]);
}

#[test]
fn distinct_values_are_distinct_and_in_range() {
    let mut rng = Rng::from_seed(5);
    for (count, range) in [(0, 5..=5), (1, 5..=5), (10, -5..=4), (10, -5..=1000), (500, 0..=1000), (1000, 1..=1000)] {
        let values: Vec<i32> = rng.distinct(count, range.clone());
        assert_eq!(values.len(), count);
        assert_eq!(values.iter().copied().collect::<HashSet<_>>().len(), count, "repeats in {values:?}");
        assert!(values.iter().all(|value| range.contains(value)));
    }
    assert_eq!(rng.distinct::<u64, _>(4, ..).into_iter().collect::<HashSet<_>>().len(), 4);
    assert_eq!(rng.distinct::<i8, _>(256, ..).into_iter().collect::<HashSet<_>>().len(), 256);
    let around_zero: Vec<i64> = rng.distinct(3, i64::MIN..=i64::MAX);
    assert_eq!(around_zero.len(), 3);
    let high: Vec<u64> = rng.distinct(2, u64::MAX - 1..);
    assert_eq!(high.iter().copied().collect::<HashSet<_>>(), HashSet::from([u64::MAX - 1, u64::MAX]));
}

/// Each of the 10 pairs out of 5 values, in each order, equally often.
#[test]
fn distinct_is_uniform() {
    let mut rng = Rng::from_seed(6);
    let mut counts = HashMap::new();
    for _ in 0..20_000 {
        *counts.entry(rng.distinct::<u8, _>(2, 0..5)).or_insert(0) += 1;
    }
    assert_eq!(counts.len(), 20);
    assert!(counts.values().all(|count| (850..1150).contains(count)), "{counts:?}");
}

#[test]
#[should_panic(expected = "cannot draw 4 different values from a range of 3")]
fn distinct_rejects_a_range_that_is_too_small() {
    let _: Vec<i32> = Rng::from_seed(0).distinct(4, 1..=3);
}

#[test]
fn partition_adds_up() {
    let mut rng = Rng::from_seed(7);
    for (total, parts, min_part) in [
        (0_i64, 0, 0),
        (0, 3, 0),
        (7, 1, 0),
        (10, 10, 1),
        (10, 3, 1),
        (1_000_000_000_000_000_000, 7, 5),
        (30, 4, -2),
        (-6, 3, -4),
    ] {
        let sizes = rng.partition(total, parts, min_part);
        assert_eq!((sizes.len(), sizes.iter().sum::<i64>()), (parts, total), "{sizes:?}");
        assert!(sizes.iter().all(|&size| size >= min_part), "{sizes:?} has a part below {min_part}");
    }
    assert_eq!(rng.partition(200_000_usize, 1000, 1).iter().sum::<usize>(), 200_000);
    assert_eq!(rng.partition(12_u8, 4, 3), [3, 3, 3, 3]);
}

/// Each of the 3 ways to split 4 into 3 positive parts, and of the 15 into 3 parts of any size, equally often.
#[test]
fn partition_is_uniform() {
    let mut rng = Rng::from_seed(8);
    for (min_part, ways) in [(1, 3), (0, 15)] {
        let mut counts = HashMap::new();
        for _ in 0..15_000 {
            *counts.entry(rng.partition(4_u32, 3, min_part)).or_insert(0) += 1;
        }
        assert_eq!(counts.len(), ways);
        let expected = 15_000 / ways;
        assert!(counts.values().all(|&count| (expected * 85 / 100..expected * 115 / 100).contains(&count)), "{counts:?}");
    }
}

#[test]
#[should_panic(expected = "cannot split 5 into 3 parts of at least 2 each")]
fn partition_rejects_parts_that_do_not_fit() {
    let _ = Rng::from_seed(0).partition(5, 3, 2);
}

#[test]
#[should_panic(expected = "cannot split 5 into 0 parts")]
fn partition_rejects_having_nowhere_to_put_the_total() {
    let _ = Rng::from_seed(0).partition(5, 0, 0);
}

#[test]
fn random_string_uses_its_alphabet() {
    let mut rng = Rng::from_seed(9);
    let text = rng.random_string(300, "ab\u{e9}");
    assert_eq!(text.chars().count(), 300);
    assert_eq!(text.chars().collect::<HashSet<_>>(), HashSet::from(['a', 'b', '\u{e9}']));
    assert_eq!(rng.random_string(0, "x"), "");
    assert_eq!(rng.random_string(3, "x"), "xxx");
}

#[test]
#[should_panic(expected = "empty alphabet")]
fn random_string_rejects_an_empty_alphabet() {
    let _ = Rng::from_seed(0).random_string(3, "");
}

#[test]
fn weighted_index_follows_the_weights() {
    let mut rng = Rng::from_seed(10);
    let mut counts = [0_u32; 4];
    for _ in 0..10_000 {
        counts[rng.weighted_index(&[1, 0, 3, 6])] += 1;
    }
    assert_eq!(counts[1], 0, "a weight of zero is never picked");
    assert!((800..1200).contains(&counts[0]) && (2700..3300).contains(&counts[2]) && (5700..6300).contains(&counts[3]), "{counts:?}");
    assert_eq!(rng.weighted_index(&[0, 0, 5]), 2);
    assert_eq!(rng.weighted_index(&[u64::MAX]), 0);
}

#[test]
#[should_panic(expected = "add up to zero")]
fn weighted_index_rejects_weights_of_zero() {
    let _ = Rng::from_seed(0).weighted_index(&[0, 0]);
}

#[test]
#[should_panic(expected = "more than a u64 holds")]
fn weighted_index_rejects_weights_that_overflow() {
    let _ = Rng::from_seed(0).weighted_index(&[u64::MAX, 1]);
}

#[test]
fn floats_stay_inside_their_range() {
    let mut rng = Rng::from_seed(11);
    for _ in 0..10_000 {
        let value: f64 = rng.random_range(-2.5..7.5);
        assert!((-2.5..7.5).contains(&value), "{value}");
        let value: f32 = rng.random_range(1.0..=1.5);
        assert!((1.0..=1.5).contains(&value), "{value}");
    }
    let average = (0..10_000).map(|_| rng.random_range(0.0..10.0)).sum::<f64>() / 10_000.0;
    assert!((4.8..5.2).contains(&average), "{average}");
    assert!((rng.random_range(3.0..=3.0_f64) - 3.0).abs() < f64::EPSILON);
    // A range a single float wide never returns its excluded end.
    let (low, high) = (1.0_f32, 1.0_f32.next_up());
    assert!((0..1000).all(|_| rng.random_range(low..high).to_bits() == low.to_bits()));
}

#[test]
#[should_panic(expected = "empty range")]
fn an_empty_range_of_floats_panics() {
    let _: f64 = Rng::from_seed(0).random_range(1.0..1.0);
}

#[test]
#[should_panic(expected = "two finite ends")]
fn a_range_of_floats_with_a_nan_panics() {
    let _: f64 = Rng::from_seed(0).random_range(0.0..f64::NAN);
}

#[test]
#[should_panic(expected = "two finite ends")]
fn an_unbounded_range_of_floats_panics() {
    let _: f64 = Rng::from_seed(0).random_range(0.0..);
}

#[test]
#[should_panic(expected = "two finite ends")]
fn an_infinite_range_of_floats_panics() {
    let _: f64 = Rng::from_seed(0).random_range(0.0..f64::INFINITY);
}

#[test]
fn chars_stay_inside_their_range() {
    let mut rng = Rng::from_seed(12);
    let letters: HashSet<char> = (0..2000).map(|_| rng.random_range('a'..='z')).collect();
    assert_eq!(letters, ('a'..='z').collect());
    assert!((0..100).all(|_| ('a'..'c').contains(&rng.random_range('a'..'c'))));
    assert_eq!(rng.random_range('q'..='q'), 'q');
    // Around the gap that the surrogates leave between the characters.
    let around_the_gap: HashSet<char> = (0..200).map(|_| rng.random_range('\u{d7fe}'..='\u{e001}')).collect();
    assert_eq!(around_the_gap, HashSet::from(['\u{d7fe}', '\u{d7ff}', '\u{e000}', '\u{e001}']));
    for _ in 0..1000 {
        let _: char = rng.random_range(..);
    }
}

#[test]
#[should_panic(expected = "empty range")]
fn an_empty_range_of_chars_panics() {
    let _ = Rng::from_seed(0).random_range('b'..'b');
}

/// Like the stream, what the helpers return for a seed can never change.
#[test]
fn the_helpers_are_frozen() {
    let mut rng = Rng::from_seed(2025);
    let got = format!(
        "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
        rng.random_vec(4, 1..=1_000_000_000_i64),
        rng.random_interval(1..=1000),
        rng.random_range_skewed(0..1000, 2),
        rng.random_range_skewed(0..1000, -2),
        rng.shuffled(0..6),
        rng.permutation(6),
        rng.distinct(4, 1..=1_000_000_000_i64),
        rng.partition(100, 5, 1),
        rng.random_string(8, "abc"),
        rng.weighted_index(&[1, 2, 3]),
        rng.random_range(0.0..1.0_f64),
        (rng.random_range(1.0..2.0_f32), rng.random_range('a'..='z')),
    );
    assert_eq!(
        got,
        "[793632689, 748387935, 770972282, 947872777] (556, 749) 858 97 [2, 5, 1, 0, 3, 4] [4, 6, 1, 3, 2, 5] \
         [531419026, 391099744, 108257916, 852877144] [1, 30, 43, 15, 11] \"abbbcbcc\" 1 0.32658700957559417 (1.7275602, 't')"
    );
}
