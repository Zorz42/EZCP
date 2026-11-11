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
    assert_eq!(st.num_generators(), 0);
    assert_eq!(st.initial_counts, Vec::<usize>::new());
    assert!(st.pick_generator(&mut Rng::from_seed(0)).is_none());
}

#[test]
fn test_subtask_with_test_adds_generators() {
    let st = three_generators();
    assert_eq!(st.num_generators(), 3);
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
fn a_validator_rejects_a_test_outside_the_constraints() {
    let st = Subtask::new(0, "t").with_test(1, |_rng| "1000000\n".to_owned()).with_validator(|test: &String| {
        assert!(test.trim().parse::<i64>().unwrap_or(0) <= 10, "too large");
    });
    let _ = st.generate_test(0, 0);
}

#[test]
fn a_validator_may_capture_its_bound() {
    let bound = 10_i64;
    let st = Subtask::new(0, "t").with_test(1, |_rng| 7_i64).with_validator(move |&test| assert!(test <= bound));
    assert_eq!(st.generate_test(0, 0), 7);
}

#[test]
fn a_fixed_test_is_its_own_generator() {
    let st = Subtask::new(0, "t").with_fixed_test("1 2\n".to_owned()).with_fixed_test("3 4\n".to_owned());
    assert_eq!(st.num_generators(), 2);
    assert_eq!(st.initial_counts, [1, 1]);
    assert_eq!((st.generate_test(0, 5), st.generate_test(1, 6)), ("1 2\n".to_owned(), "3 4\n".to_owned()));
}

#[test]
fn input_generation_normalises_whitespace_only_when_asked() {
    let st = Subtask::new(0, "t").with_fixed_test(" 1  2 ".to_owned());
    assert_eq!(st.generate_input(0, 0, true), "1 2\n");
    assert_eq!(st.generate_input(0, 0, false), " 1  2 ");
}
