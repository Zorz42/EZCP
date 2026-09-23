use crate::rng::Rng;
use crate::{Error, Result};

/// What a task run does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// Generate the tests, write them to files and archive them.
    #[default]
    Files,
    /// Like [`Mode::Files`], but each file holds the [stub](crate::Stub) that
    /// rebuilds the test instead of the test itself.
    Seeds,
    /// Read stubs from stdin, one per line, and write the raw test data each one
    /// stands for to stdout.
    Serve,
}

/// Where the master seed comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SeedChoice {
    /// [`DEFAULT_SEED`](crate::DEFAULT_SEED).
    #[default]
    Default,
    /// A given seed.
    Fixed(u64),
    /// A new seed on every run.
    Random,
}

impl SeedChoice {
    /// Returns the seed, `default` for [`SeedChoice::Default`].
    #[must_use]
    pub fn resolve(self, default: u64) -> u64 {
        match self {
            Self::Default => default,
            Self::Fixed(seed) => seed,
            Self::Random => Rng::from_entropy().next_u64(),
        }
    }
}

/// The parsed command line of a task binary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CliOptions {
    /// The mode to run in.
    pub mode: Mode,
    /// The `--seed` value, if given.
    pub seed: Option<SeedChoice>,
    /// Whether `--help` was given.
    pub help: bool,
}

pub const USAGE: &str = "\
Usage: <task> [options]

Modes:
  (no mode given)   Generate the tests, write them to files and pack them into a
                    zip archive.
  --seeds           Generate and verify the tests exactly as above, but write
                    each test file as the seed that rebuilds it rather than as
                    the test data itself.
  --serve           Read those seeds on stdin, one per line, and write out the
                    test data each of them stands for.

Options:
  --seed <value>    Master seed for test generation: a decimal number, a 0x-prefixed
                    hexadecimal number, or `random` for an unpredictable one.
                    Ignored by --serve, which takes each test's seed from the stub.
  -h, --help        Print this text.";

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
    /// Parses arguments, not including the program name.
    pub fn parse<I: IntoIterator<Item = S>, S: AsRef<str>>(arguments: I) -> Result<Self> {
        let mut options = Self::default();
        let mut mode_argument: Option<String> = None;
        let mut arguments = arguments.into_iter();

        while let Some(argument) = arguments.next() {
            let argument = argument.as_ref();
            match argument {
                "--seeds" | "--serve" | "--files" => {
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
        // A non-UTF-8 argument is never a valid one, and then gets the clearer
        // "unknown argument" error.
        let arguments = std::env::args_os().skip(1).map(|argument| argument.to_string_lossy().into_owned()).collect::<Vec<_>>();
        Self::parse(arguments)
    }
}
