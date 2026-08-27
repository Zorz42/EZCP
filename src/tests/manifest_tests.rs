#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod manifest_tests {
    use crate::manifest::{Manifest, ManifestSubtask, ManifestTest, stable_hash};

    fn sample() -> Manifest {
        Manifest {
            task: "Coupon".to_owned(),
            seed: 0xdead_beef_1234_5678,
            trim_whitespace: true,
            time_limit: 5000,
            subtasks: vec![
                ManifestSubtask {
                    index: 0,
                    points: 10,
                    name: "n = 1".to_owned(),
                    tests: vec![ManifestTest {
                        index_in_subtask: 0,
                        global_index: 0,
                        generator: 0,
                        seed: u64::MAX,
                        input_file: "test.01.001.in".to_owned(),
                        output_file: "test.01.001.out".to_owned(),
                        input_hash: 1,
                        output_hash: 2,
                    }],
                },
                ManifestSubtask {
                    index: 1,
                    points: 90,
                    name: "no constraints".to_owned(),
                    tests: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn a_manifest_survives_a_round_trip() {
        let manifest = sample();
        let parsed = Manifest::from_json_string(&manifest.to_json_string()).unwrap();
        assert_eq!(manifest, parsed);
    }

    /// A seed uses all 64 bits, and JSON numbers are doubles in many readers.
    #[test]
    fn large_seeds_keep_every_bit() {
        let manifest = sample();
        let text = manifest.to_json_string();
        assert!(text.contains("\"ffffffffffffffff\""), "the seed was not written as hex: {text}");
        assert_eq!(Manifest::from_json_string(&text).unwrap().subtasks[0].tests[0].seed, u64::MAX);
    }

    #[test]
    fn foreign_json_is_rejected() {
        let err = Manifest::from_json_string(r#"{"format":"something-else","version":1}"#).unwrap_err();
        assert!(err.to_string().contains("not an EZCP seed manifest"), "{err}");
    }

    #[test]
    fn a_future_version_is_rejected() {
        let err = Manifest::from_json_string(r#"{"format":"ezcp-seeds","version":99,"task":"x","seed":"0","trim_whitespace":true,"time_limit":1,"subtasks":[]}"#).unwrap_err();
        assert!(err.to_string().contains("version 99"), "{err}");
    }

    #[test]
    fn broken_json_is_rejected() {
        Manifest::from_json_string("{").unwrap_err();
        Manifest::from_json_string("[]").unwrap_err();
    }

    #[test]
    fn a_missing_field_is_named() {
        let err = Manifest::from_json_string(r#"{"format":"ezcp-seeds","version":1,"task":"x"}"#).unwrap_err();
        assert!(err.to_string().contains("seed"), "{err}");
    }

    #[test]
    fn tests_can_be_looked_up() {
        let manifest = sample();
        assert_eq!(manifest.find_test(0, 0).unwrap().input_file, "test.01.001.in");
        assert!(manifest.find_test(0, 1).is_none());
        assert!(manifest.find_test(9, 0).is_none());
        assert_eq!(manifest.num_tests(), 1);
    }

    /// The hash is compared against one computed by another build, so its values
    /// are as much of a promise as the random stream is.
    #[test]
    fn the_hash_is_frozen() {
        assert_eq!(stable_hash(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(stable_hash("a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(stable_hash("foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn the_hash_separates_different_inputs() {
        assert_ne!(stable_hash("1 2\n"), stable_hash("1 3\n"));
        assert_ne!(stable_hash("1 2\n"), stable_hash("1 2"));
    }
}
