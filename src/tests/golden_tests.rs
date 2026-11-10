//! Pins the exact bytes that seeds produce. A published stub names a generator
//! and a seed, so anything that changes these values silently changes, or
//! invalidates, every test set that was ever generated.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod golden_tests {
    use crate::generators::Graph;
    use crate::rng::Rng;
    use crate::stub::stable_hash;
    use crate::{Mode, Subtask, Task, ToOutput, array_generator};
    use log::LevelFilter;
    use std::path::Path;
    use tempfile::TempDir;

    fn graph_hash(seed: u64, build: fn(&mut Rng) -> Graph) -> u64 {
        stable_hash(&build(&mut Rng::from_seed(seed)).to_output())
    }

    #[test]
    fn graph_constructors_are_frozen() {
        let builders: [(&str, fn(&mut Rng) -> Graph, [u64; 2]); 11] = [
            ("empty", |rng| Graph::new_empty(rng, 7), [0x835d_37e1_d55b_8c74, 0x835d_37e1_d55b_8c74]),
            ("full", |rng| Graph::new_full(rng, 12), [0xabdd_511e_357a_f049, 0x634f_5c2e_6874_d579]),
            ("random sparse", |rng| Graph::new_random(rng, 40, 60), [0x2364_a7fd_2b86_2eb4, 0x95cd_cdbb_9d67_af29]),
            ("random dense", |rng| Graph::new_random(rng, 20, 170), [0x5174_f4c6_f47a_2027, 0xe822_4c3e_a9a5_a237]),
            ("path", |rng| Graph::new_random_path(rng, 30), [0x95ff_e7cf_e9fe_a57e, 0xd121_7b8b_900a_82aa]),
            ("tree", |rng| Graph::new_random_tree(rng, 30), [0x9512_c244_fdf3_5421, 0x729f_8087_a426_e8c7]),
            ("deep tree", |rng| Graph::new_random_deep_tree(rng, 30), [0x8c32_48d3_ef0a_6321, 0xa641_cb00_0e36_ff57]),
            ("connected", |rng| Graph::new_random_connected(rng, 25, 60), [0xf04d_8801_a948_b09e, 0x04af_ffd7_f996_cb1a]),
            ("connected dense", |rng| Graph::new_random_connected(rng, 12, 60), [0x25db_9dc8_c120_040a, 0x7209_316b_cc14_f53a]),
            ("bipartite sparse", |rng| Graph::new_random_bipartite(rng, 30, 40), [0xe9ca_0643_5b1f_449d, 0x4b4e_5b8a_d047_a51e]),
            ("bipartite dense", |rng| Graph::new_random_bipartite(rng, 16, 60), [0x41b8_2259_b44f_6086, 0xd9ae_9fb4_f476_67e1]),
        ];
        let got: Vec<_> = builders.iter().map(|&(name, build, _)| (name, [graph_hash(1, build), graph_hash(2, build)])).collect();
        let expected: Vec<_> = builders.iter().map(|&(name, _, expected)| (name, expected)).collect();
        assert_eq!(got, expected, "\n{got:#018x?}");
    }

    #[test]
    fn array_generators_are_frozen() {
        let hash = |seed| stable_hash(&array_generator(1..=50, -1000..=1000)(&mut Rng::from_seed(seed)).to_output());
        let got = [hash(1), hash(2)];
        assert_eq!(got, [0x5135_7786_73ff_41de, 0x7c69_ee09_5043_7537], "[{:#018x}, {:#018x}]", got[0], got[1]);
    }

    const DOUBLE: &str = "#include <iostream>\nint main() { long long n; std::cin >> n; std::cout << 2 * n << '\\n'; }";

    /// A C++ program that doubles `n`, except where `wrong_when` holds.
    fn wrong_when(condition: &str) -> String {
        format!("#include <iostream>\nint main() {{ long long n; std::cin >> n; std::cout << (({condition}) ? 2 * n + 1 : 2 * n) << '\\n'; }}")
    }

    /// Hashes the name and contents of every test file, in name order.
    fn fingerprint(path: &Path) -> (usize, u64) {
        let mut files = std::fs::read_dir(path.join("tests"))
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name().to_string_lossy().into_owned(), std::fs::read_to_string(entry.path()).unwrap())
            })
            .collect::<Vec<_>>();
        files.sort();
        let listing: Vec<&str> = files.iter().flat_map(|(name, contents)| [name.as_str(), "\n", contents.as_str(), "\u{0}"]).collect();
        (files.len(), stable_hash(&listing.concat()))
    }

    /// Exercises duplicates, stress tests, generators that only hunt, and
    /// partial solutions that are broken often, rarely and never.
    fn scenario(path: &Path) -> Task<String> {
        Task::new("Golden", path)
            .with_debug_level(LevelFilter::Off)
            .with_solution_source(DOUBLE)
            .with_subtask(
                Subtask::new(20, "small")
                    .with_test(4, |rng| format!("{}\n", rng.random_range(1..=6)))
                    .with_test(3, |rng| format!("{}\n", rng.random_range(1..=3)))
                    .with_test(2, |_rng| "5\n".to_owned())
                    .with_stress_tests(3),
            )
            .with_subtask(
                Subtask::new(30, "medium")
                    .with_test(5, |rng| format!("{}\n", rng.random_range(1..=1000)))
                    .with_test(0, |rng| format!("{}\n", rng.random_range(900..=1000)))
                    .with_min_failures(4),
            )
            .with_subtask(
                Subtask::new(50, "large")
                    .with_test(6, |rng| format!("{}\n", rng.random_range(1..=1_000_000)))
                    .with_test(2, |rng| format!("{}\n", rng.random_range(999_000..=1_000_000))),
            )
            .with_partial_solution("small only", &wrong_when("n > 6"), &[0])
            .with_partial_solution("not large", &wrong_when("n > 1000"), &[0, 1])
            .with_partial_solution("multiples of 7", &wrong_when("n % 7 == 0 && n > 6"), &[0])
            .with_partial_solution("rarely wrong", &wrong_when("n > 1000 && n % 40 == 3"), &[0, 1])
            .with_min_failures(3)
            .with_max_tries(400)
    }

    #[test]
    fn a_seed_keeps_producing_the_same_test_set() {
        let runs = [(Mode::Files, None), (Mode::Files, Some(7)), (Mode::Seeds, None), (Mode::Seeds, Some(7))];
        let got = runs.map(|(mode, seed)| {
            let dir = TempDir::new().unwrap();
            let task = scenario(dir.path());
            seed.map_or(task, |seed| scenario(dir.path()).with_seed(seed)).run_mode(mode).unwrap();
            fingerprint(dir.path())
        });
        let expected = [(54, 0x66a1_d9c7_de4b_c9f6), (56, 0x8855_1510_2763_4662), (54, 0x1546_cee4_2ff5_2aba), (56, 0xd7d2_d940_d624_7fb7)];
        assert_eq!(got, expected, "\n{got:#018x?}");
    }

    /// Candidates are drawn and run several at a time, as many as there are
    /// jobs, but which of them become tests may not depend on that.
    #[test]
    fn the_test_set_does_not_depend_on_the_number_of_jobs() {
        for jobs in [1, 2, 3, 7, 32] {
            let dir = TempDir::new().unwrap();
            scenario(dir.path()).with_jobs(jobs).with_fail_fast(jobs % 2 == 1).run_mode(Mode::Files).unwrap();
            assert_eq!(fingerprint(dir.path()), (54, 0x66a1_d9c7_de4b_c9f6), "with {jobs} jobs");
        }
    }
}
