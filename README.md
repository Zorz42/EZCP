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

Running it writes `sum/tests/`, `sum/tests.zip`, `sum/results.txt` and
`sum/seeds.json`. See `examples/` for complete tasks.

**A generator must take all of its randomness from the `rng` it is given.** A
test is identified by its generator plus its seed, so that pair has to always
rebuild the same bytes.

## Command line

The task binary takes the mode from its arguments (`--help` lists them):

| Arguments | What it does |
| --- | --- |
| *(none)* | Generate the tests, write them to files, archive them, write `seeds.json`. |
| `--seeds` | Same pipeline, but keep only `seeds.json` — no test files, no archive. |
| `--serve` | Answer requests on stdin with test data rebuilt from `seeds.json`. |
| `--seed <value>` | Master seed: a number, `0x`-prefixed hex, or `random`. Ignored by `--serve`. |

### `--seeds`

Every test is still generated and verified; only the manifest is kept, so a task
can have far more tests than there is room to store (22 MB of files vs. a 20 kB
manifest on `examples/example2`). Each kept test is then rebuilt from its seed
ten times and compared, so a generator that is not faithful to its `Rng` fails
the run instead of leaving a manifest that lies.
`Task::with_reproducibility_checks(n)` changes the count; `0` turns it off.

### `--serve`

Reads one JSON request per line and answers each with the **raw bytes** of one
half of a test — byte for byte the file a normal run would have written, with no
framing, escaping or added newline:

```console
$ echo '{"command":"test","subtask":0,"test":0,"part":"input"}' | ./task --serve > test.in
$ echo '{"command":"test","subtask":0,"test":0,"part":"output"}' | ./task --serve
1
```

| Request | Meaning |
| --- | --- |
| `{"command":"test","subtask":0,"test":3,"part":"input"}` | Half of the test the manifest lists there. `"part"` is `"input"` or `"output"` and is required. |
| `{"command":"seed","subtask":0,"generator":1,"seed":"a1b2...","part":"output"}` | Half of a test built from a generator and seed directly, listed or not. Seeds are hex **strings**, since a JSON number loses the low bits. |
| `{"command":"info"}` | The task and its subtasks, as one JSON line — the only answer that is not raw. |
| `{"command":"quit"}` | Stop serving. |

Asking for the input is the cheap request; the solution only runs for `"output"`.
A request that cannot be answered writes nothing to stdout, reports the error on
stderr and ends the session, since raw bytes leave no way to signal a failure in
the stream. A manifest from another task, or a generator that no longer produces
the test recorded for a seed, is refused rather than served.

## Notes

- Prefer deriving `ToOutput` on a struct over formatting output strings by hand.
- `Graph`'s constructors take the generator's `rng`, even the ones with no
  randomness of their own — writing a graph shuffles its edges.
- Solutions get a 512 MB stack, and time limits are measured in CPU time.
