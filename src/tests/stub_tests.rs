#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod stub_tests {
    use crate::stub::{Part, Stub, stable_hash};

    fn sample() -> Stub {
        Stub {
            subtask: 2,
            generator: 1,
            seed: u64::MAX,
            part: Part::Output,
            hash: Some(0x1234),
        }
    }

    #[test]
    fn a_stub_survives_a_round_trip() {
        let stub = sample();
        assert_eq!(Stub::parse(&stub.to_line()).unwrap(), stub);
    }

    /// A stub is one line, because a test file holds exactly one of them and the
    /// server reads them a line at a time.
    #[test]
    fn a_stub_is_a_single_line() {
        let line = sample().to_line();
        assert_eq!(line.lines().count(), 1);
        assert!(line.ends_with('\n'));
    }

    /// The seed uses all 64 bits, which is why it is written as a string: a
    /// reader that parses JSON numbers as doubles would lose the low ones.
    #[test]
    fn the_full_range_of_a_seed_survives() {
        assert!(sample().to_line().contains("\"ffffffffffffffff\""));
        assert_eq!(Stub::parse(&sample().to_line()).unwrap().seed, u64::MAX);
    }

    /// A request written by hand may leave the hash out, and may write the seed
    /// as a plain number.
    #[test]
    fn the_hash_is_optional_and_a_small_seed_may_be_a_number() {
        let stub = Stub::parse(r#"{"subtask":0,"generator":0,"seed":17,"part":"input"}"#).unwrap();
        assert_eq!(stub.seed, 17);
        assert_eq!(stub.part, Part::Input);
        assert_eq!(stub.hash, None);
    }

    #[test]
    fn nonsense_is_refused() {
        for bad in [
            "{",
            "[]",
            r#"{"subtask":0,"generator":0,"seed":"1"}"#,
            r#"{"subtask":0,"generator":0,"seed":"1","part":"sideways"}"#,
            r#"{"subtask":0,"generator":0,"seed":"nothex","part":"input"}"#,
            r#"{"subtask":-1,"generator":0,"seed":"1","part":"input"}"#,
            r#"{"subtask":0,"generator":0,"seed":"1","part":"input","hash":"nothex"}"#,
        ] {
            assert!(Stub::parse(bad).is_err(), "{bad} should not parse");
        }
    }

    /// The hash is compared against one computed by a different build, possibly
    /// years later, so its value is part of the format.
    #[test]
    fn the_hash_is_fixed_by_its_specification() {
        assert_eq!(stable_hash(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(stable_hash("a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(stable_hash("foobar"), 0x8594_4171_f739_67e8);
    }
}
