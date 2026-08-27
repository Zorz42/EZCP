#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod mode_tests {
    use crate::mode::{CliOptions, Mode, SeedChoice};

    #[test]
    fn no_arguments_means_files_mode() {
        let options = CliOptions::parse::<[&str; 0], &str>([]).unwrap();
        assert_eq!(options.mode, Mode::Files);
        assert_eq!(options.seed, None);
        assert!(!options.help);
    }

    #[test]
    fn modes_are_recognised() {
        assert_eq!(CliOptions::parse(["--seeds"]).unwrap().mode, Mode::Seeds);
        assert_eq!(CliOptions::parse(["--serve"]).unwrap().mode, Mode::Serve);
        assert_eq!(CliOptions::parse(["--files"]).unwrap().mode, Mode::Files);
    }

    #[test]
    fn two_different_modes_are_rejected() {
        let err = CliOptions::parse(["--seeds", "--serve"]).unwrap_err();
        assert!(err.to_string().contains("cannot both be given"), "{err}");
    }

    #[test]
    fn the_same_mode_twice_is_fine() {
        assert_eq!(CliOptions::parse(["--seeds", "--seeds"]).unwrap().mode, Mode::Seeds);
    }

    #[test]
    fn seeds_are_parsed_in_every_spelling() {
        assert_eq!(CliOptions::parse(["--seed", "42"]).unwrap().seed, Some(SeedChoice::Fixed(42)));
        assert_eq!(CliOptions::parse(["--seed=42"]).unwrap().seed, Some(SeedChoice::Fixed(42)));
        assert_eq!(CliOptions::parse(["--seed", "0xff"]).unwrap().seed, Some(SeedChoice::Fixed(255)));
        assert_eq!(CliOptions::parse(["--seed", "random"]).unwrap().seed, Some(SeedChoice::Random));
        assert_eq!(CliOptions::parse(["--seed", "18446744073709551615"]).unwrap().seed, Some(SeedChoice::Fixed(u64::MAX)));
    }

    #[test]
    fn a_bad_seed_is_rejected() {
        CliOptions::parse(["--seed", "nonsense"]).unwrap_err();
        CliOptions::parse(["--seed", "-1"]).unwrap_err();
        CliOptions::parse(["--seed"]).unwrap_err();
    }

    #[test]
    fn unknown_arguments_are_rejected() {
        // Task binaries are run by hand and by scripts; an ignored typo would mean
        // silently generating something other than what was asked for.
        let err = CliOptions::parse(["--nonsense"]).unwrap_err();
        assert!(err.to_string().contains("--nonsense"), "{err}");
    }

    #[test]
    fn help_is_recognised() {
        assert!(CliOptions::parse(["--help"]).unwrap().help);
        assert!(CliOptions::parse(["-h"]).unwrap().help);
    }

    #[test]
    fn a_fixed_seed_resolves_to_itself() {
        assert_eq!(SeedChoice::Fixed(7).resolve(1), 7);
        assert_eq!(SeedChoice::Default.resolve(1), 1);
        assert_ne!(SeedChoice::Random.resolve(1), SeedChoice::Random.resolve(1));
    }
}
