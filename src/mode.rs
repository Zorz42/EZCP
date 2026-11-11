use crate::rng::Rng;
use crate::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// Write the tests and archive them.
    #[default]
    Files,
    /// As `Files`, but each file holds the one-line stub that rebuilds it.
    Seeds,
    /// Answer stubs on stdin with the test data they stand for.
    Serve,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeedChoice {
    Fixed(u64),
    Random,
}

impl SeedChoice {
    pub fn resolve(self) -> u64 {
        match self {
            Self::Fixed(seed) => seed,
            Self::Random => Rng::from_entropy().next_u64(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CliOptions {
    pub mode: Mode,
    pub seed: Option<SeedChoice>,
    pub jobs: Option<usize>,
    pub fail_fast: bool,
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
  --jobs <count>    How many solutions run at once. The default is the number of
                    CPU cores, but at most 4. The tests do not depend on it.
  --fail-fast       Stop running a partial solution on a subtask it is meant to
                    fail at its first failed test there.
  -h, --help        Print this text.";

const fn invalid(details: String) -> Error {
    Error::InvalidArguments { details }
}

fn parse_seed(value: &str) -> Result<SeedChoice> {
    if value == "random" {
        return Ok(SeedChoice::Random);
    }
    let parsed = value.strip_prefix("0x").map_or_else(|| value.parse().ok(), |hex| u64::from_str_radix(hex, 16).ok());
    parsed
        .map(SeedChoice::Fixed)
        .ok_or_else(|| invalid(format!("\"{value}\" is not a seed; give a number, a 0x-prefixed hexadecimal number, or `random`")))
}

fn parse_jobs(value: &str) -> Result<usize> {
    value
        .parse()
        .ok()
        .filter(|&jobs| jobs > 0)
        .ok_or_else(|| invalid(format!("\"{value}\" is not a number of jobs; give a positive number")))
}

impl CliOptions {
    /// Without the program name.
    pub fn parse<I: IntoIterator<Item = S>, S: AsRef<str>>(arguments: I) -> Result<Self> {
        let mut options = Self::default();
        let mut mode_argument: Option<String> = None;
        let mut arguments = arguments.into_iter();

        while let Some(argument) = arguments.next() {
            let argument = argument.as_ref();
            match argument {
                "--seeds" | "--serve" | "--files" => {
                    if let Some(first) = mode_argument.replace(argument.to_owned())
                        && first != argument
                    {
                        return Err(invalid(format!("{first} and {argument} cannot both be given")));
                    }
                    options.mode = match argument {
                        "--seeds" => Mode::Seeds,
                        "--serve" => Mode::Serve,
                        _ => Mode::Files,
                    };
                }
                "--seed" => options.seed = Some(parse_seed(arguments.next().ok_or_else(|| invalid("--seed needs a value".to_owned()))?.as_ref())?),
                "--jobs" => options.jobs = Some(parse_jobs(arguments.next().ok_or_else(|| invalid("--jobs needs a value".to_owned()))?.as_ref())?),
                "--fail-fast" => options.fail_fast = true,
                "-h" | "--help" => options.help = true,
                _ => {
                    if let Some(value) = argument.strip_prefix("--seed=") {
                        options.seed = Some(parse_seed(value)?);
                    } else if let Some(value) = argument.strip_prefix("--jobs=") {
                        options.jobs = Some(parse_jobs(value)?);
                    } else {
                        return Err(invalid(format!("unknown argument \"{argument}\"")));
                    }
                }
            }
        }

        Ok(options)
    }

    pub fn from_env() -> Result<Self> {
        // Lossy: a non-UTF-8 argument is never valid, and gets the "unknown argument" error.
        let arguments = std::env::args_os().skip(1).map(|argument| argument.to_string_lossy().into_owned()).collect::<Vec<_>>();
        Self::parse(arguments)
    }
}
