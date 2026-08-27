//! What a task binary does when it is run, and how the command line selects it.

use crate::rng::Rng;
use crate::{Error, Result};

/// The three ways a task can be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// Generate the tests and write them out as files, then pack them into a zip.
    ///
    /// This is what a task does when it is run without arguments. A seed manifest
    /// is written alongside the files, so the same tests can be served on demand
    /// later without generating them again.
    #[default]
    Files,
    /// Generate and verify exactly as [`Mode::Files`] does, but keep only the seed
    /// manifest.
    ///
    /// Every test is still generated, run against the official solution and used
    /// to hunt for counterexamples; none of them are written to disk. This is how
    /// a task can have far more tests than there is room to store.
    Seeds,
    /// Serve tests over stdin and stdout, rebuilding them from the manifest on
    /// demand.
    ///
    /// Nothing is generated up front: the task's solution is compiled, the
    /// manifest is read, and each request is answered with the exact bytes the
    /// corresponding test file would have held.
    Serve,
}

/// Where the master seed comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SeedChoice {
    /// The task's own default, which does not change between runs.
    #[default]
    Default,
    /// A seed given on the command line or in the task definition.
    Fixed(u64),
    /// A fresh seed from the operating system, reported so the run can be
    /// repeated.
    Random,
}

impl SeedChoice {
    /// Resolves the choice into an actual seed.
    #[must_use]
    pub fn resolve(self, default: u64) -> u64 {
        match self {
            Self::Default => default,
            Self::Fixed(seed) => seed,
            Self::Random => Rng::from_entropy().next_u64(),
        }
    }
}

/// What the command line asked a task binary to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CliOptions {
    pub mode: Mode,
    pub seed: Option<SeedChoice>,
    /// Set by `--help`, which prints the usage text and does nothing else.
    pub help: bool,
}

/// The usage text, printed by `--help`.
pub const USAGE: &str = "\
Usage: <task> [options]

Modes:
  (no mode given)   Generate the tests, write them to files and pack them into a
                    zip archive. A seed manifest is written as well.
  --seeds           Generate and verify the tests exactly as above, but write
                    only the seed manifest, no test files and no archive.
  --serve           Read requests on stdin and answer them with the tests named
                    by the seed manifest, one JSON object per line.

Options:
  --seed <value>    Master seed for test generation: a decimal number, a 0x-prefixed
                    hexadecimal number, or `random` for an unpredictable one.
                    Ignored by --serve, which takes each test's seed from the manifest.
  -h, --help        Print this text.";

/// Parses a seed given on the command line.
fn parse_seed(value: &str) -> Result<SeedChoice> {
    if value == "random" {
        return Ok(SeedChoice::Random);
    }

    let parsed = value.strip_prefix("0x").map_or_else(|| value.parse::<u64>().ok(), |hex| u64::from_str_radix(hex, 16).ok());

    parsed.map(SeedChoice::Fixed).ok_or_else(|| Error::InvalidArguments {
        details: format!("\"{value}\" is not a seed; give a number, a 0x-prefixed hexadecimal number, or `random`"),
    })
}

impl CliOptions {
    /// Parses the arguments a task binary was started with.
    ///
    /// `arguments` must not include the name of the program itself.
    pub fn parse<I: IntoIterator<Item = S>, S: AsRef<str>>(arguments: I) -> Result<Self> {
        let mut options = Self::default();
        let mut mode_argument: Option<String> = None;
        let mut arguments = arguments.into_iter();

        while let Some(argument) = arguments.next() {
            let argument = argument.as_ref();
            match argument {
                "--seeds" | "--serve" | "--files" => {
                    // Two modes in one command line is a mistake worth reporting:
                    // silently keeping the last one would generate a whole set of
                    // tests the caller did not ask for.
                    if let Some(first) = &mode_argument
                        && first != argument
                    {
                        return Err(Error::InvalidArguments {
                            details: format!("{first} and {argument} cannot both be given"),
                        });
                    }
                    options.mode = match argument {
                        "--seeds" => Mode::Seeds,
                        "--serve" => Mode::Serve,
                        _ => Mode::Files,
                    };
                    mode_argument = Some(argument.to_owned());
                }
                "--seed" => {
                    let value = arguments.next().ok_or_else(|| Error::InvalidArguments {
                        details: "--seed needs a value".to_owned(),
                    })?;
                    options.seed = Some(parse_seed(value.as_ref())?);
                }
                "-h" | "--help" => options.help = true,
                _ => {
                    if let Some(value) = argument.strip_prefix("--seed=") {
                        options.seed = Some(parse_seed(value)?);
                    } else {
                        return Err(Error::InvalidArguments {
                            details: format!("unknown argument \"{argument}\""),
                        });
                    }
                }
            }
        }

        Ok(options)
    }

    /// Parses the arguments of the running process.
    pub fn from_env() -> Result<Self> {
        // Lossy rather than rejecting: an argument that is not valid UTF-8 is
        // never one of ours, and the error for an unknown argument says more than
        // one about encoding would.
        let arguments = std::env::args_os().skip(1).map(|argument| argument.to_string_lossy().into_owned()).collect::<Vec<_>>();
        Self::parse(arguments)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{CliOptions, Mode, SeedChoice};

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
