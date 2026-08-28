# EZCP

A Rust framework for generating test data for competitive programming tasks.

You describe a task — an official solution, subtasks, generators, optionally
partial solutions and a checker — and EZCP generates the inputs, runs the
solution to get the correct outputs, hunts for cases that break the partial
solutions, and writes everything out.

## Requirements

Rust 1.88+ and a C++ compiler (`g++`/`clang++`; on Windows a MinGW-w64
toolchain). It is looked up on `PATH`; set `GCC_PATH` to choose one.

## Quick start

```toml
[dependencies]
ezcp = "0.5"
```

```rust
use std::path::PathBuf;

const SOLUTION: &str = r"
#include <iostream>
int main() { int a, b; std::cin >> a >> b; std::cout << a + b << std::endl; }
";

fn main() -> ezcp::Result<()> {
    ezcp::Task::new("Sum", &PathBuf::from("sum"))
        .with_solution_source(SOLUTION)
        .with_subtask(ezcp::Subtask::new(100, "a, b <= 1000").with_test(10, |rng| {
            format!("{} {}\n", rng.random_range(0..=1000), rng.random_range(0..=1000))
        }))
        .run()
}
```

Running it writes `sum/tests/`, `sum/tests.zip` and `sum/results.txt`. See
`examples/` for complete tasks.

**A generator must take all of its randomness from the `rng` it is given.** A
test is identified by its generator plus its seed, so that pair has to always
rebuild the same bytes.

## Command line

The task binary takes the mode from its arguments (`--help` lists them):

| Arguments | What it does |
| --- | --- |
| *(none)* | Generate the tests, write them to files, archive them. |
| `--seeds` | The same, except each test file holds the seed that rebuilds the test instead of the test data. |
| `--serve` | Read such a file on stdin and write out the test data it stands for. |
| `--seed <value>` | Master seed: a number, `0x`-prefixed hex, or `random`. Ignored by `--serve`. |

### `--seeds`

Every test is still generated, run against the official solution and used to hunt
for cases that break the partial solutions — only the data is left out. Each file
holds one line naming the generator and seed that produce it, plus a hash of what
they produced, so the test set of `examples/example2` is 12 kB instead of 22 MB. The file names, the layout and `tests.zip` are exactly what a normal
run produces.

Before writing anything, each kept test is rebuilt from its seed ten times and
compared, so a generator that is not faithful to its `Rng` fails the run instead
of leaving behind a seed that does not reproduce its test.
`Task::with_reproducibility_checks(n)` changes the count; `0` turns it off.

### `--serve`

Pipe a file written by `--seeds` in, and the test data it stands for comes out —
byte for byte the file a normal run would have written, with nothing added:

```console
$ ./task --serve < tests/test.01.001.in > test.in
$ ./task --serve < tests/test.01.001.out > test.out
```

Several can be fed in at once, one per line, and the answers come back in order.
Rebuilding an input never runs the official solution, so it costs nothing but the
generator; rebuilding an output does.

Nothing frames the data on the way out, which leaves no way to report a failure
in the stream. A file that cannot be rebuilt therefore produces no output at all:
the error goes to stderr and the run ends non-zero. That covers a generator that
has changed since the seeds were written — the hash catches it, and refusing is
better than handing out a test that was never verified.

## Notes

- Prefer deriving `ToOutput` on a struct over formatting output strings by hand.
- `Graph`'s constructors take the generator's `rng`, even the ones with no
  randomness of their own — writing a graph shuffles its edges.
- Solutions get a 512 MB stack, and time limits are measured in CPU time.
