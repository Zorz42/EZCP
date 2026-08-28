//! What a task binary does when it is run, and how the command line selects it.

use crate::rng::Rng;
use crate::{Error, Result};

/// The three ways a task can be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// Generate the tests and write them out as files, then pack them into a zip.
    ///
    /// This is what a task does when it is run without arguments.
    #[default]
    Files,
    /// Generate and verify exactly as [`Mode::Files`] does, but write each test
    /// file as the [stub](crate::Stub) that rebuilds it instead of as the test.
    ///
    /// Every test is still generated, run against the official solution and used
    /// to hunt for counterexamples; none of the data is written to disk. This is
    /// how a task can have far more tests than there is room to store. The file
    /// names, the layout and the archive are the same as file mode's.
    Seeds,
    /// Turn stubs back into tests, on stdin and stdout.
    ///
    /// Nothing is generated up front. Every line of stdin is a stub, as written
    /// by [`Mode::Seeds`], and the answer is the raw bytes it stands for: pipe a
    /// stub file in and the test file a normal run would have written comes back
    /// out.
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
    /// Which of the three modes to run in.
    pub mode: Mode,
    /// The seed from `--seed`, or `None` if the argument was not given, in which
    /// case whatever the task itself was configured with applies.
    pub seed: Option<SeedChoice>,
    /// Set by `--help`, which prints the usage text and does nothing else.
    pub help: bool,
}

/// The usage text, printed by `--help`.
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
