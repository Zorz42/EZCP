# EZCP
A Rust framework to easily create tasks for competitive programming.

Features:
- Generate test inputs and save them into files.
- Generate correct outputs.
- Make a solution checker if there are multiple valid solutions.
- Graph generator.
- Array generator.
- Add a partial solution and specify which subtasks it should pass.
- Automatically search for testcases that break all bad solutions.
- Automatically archive all test files into a zip file.
- Keep a whole task's tests as a few kilobytes of seeds and rebuild any of them
  on demand, byte for byte.

See `examples/` for complete, runnable tasks.

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

This writes the tests to `sum/tests/`, packs them into `sum/tests.zip` and leaves
a per-subtask summary in `sum/results.txt`. It also writes `sum/seeds.json`, which
records how every test was made.

## Generators and randomness

A generator is handed a seeded `Rng` and has to take **all** of its randomness
from it. That is what makes a test reproducible: a test is identified by the
generator that made it and the seed it was run with, and that pair always gives
back the same bytes — on another machine, in another year, in another build.

A generator that draws from somewhere else, or that captures a random value from
the surrounding code, still compiles and still produces tests. It just produces
tests that cannot be rebuilt. Nothing in the type system can prevent that, so
`--seeds` goes looking for it instead — see below.

```rust
// Right: everything comes from `rng`.
.with_test(5, |rng| format!("{}\n", rng.random_range(1..=1000)))

// Wrong: `n` is drawn once, when the task is described, and baked in.
let n = some_other_rng.random_range(1..=1000);
.with_test(5, move |_rng| format!("{n}\n"))
```

Two runs of the same task produce the same tests, because the master seed is
fixed. Pass `--seed random` (or `--seed 12345`, or `Task::with_random_seed()`)
to explore different ones; whichever seed was used is recorded in the manifest,
so a run worth keeping can be repeated.

## On-demand tests

A task binary has three modes, chosen on the command line, so the one binary
covers all of them. `--help` describes them.

| Command | What it does |
| --- | --- |
| *(no arguments)* | Generate the tests, write them to files, archive them, and write the seed manifest. |
| `--seeds` | Generate and verify exactly the same tests, but keep only the manifest. |
| `--serve` | Answer requests on stdin with the tests the manifest names. |

`--seeds` runs the identical pipeline: every test is generated, checked against
the official solution, and used to hunt for counterexamples that break the
partial solutions. Nothing reaches the disk except the manifest, so a task can
have far more tests than there is room to store. On `examples/example2` that is
22 MB of test files against a 20 kB manifest.

It then rebuilds each of the finished tests from its seed ten times over and
checks that it comes out identical every time. A seed is only worth recording if
it really does reproduce the test, and in seed mode there is no file to fall back
on, so a generator that is not faithful to its `Rng` fails the run instead of
leaving behind a manifest that lies:

```console
$ ./task --seeds
Error: Generator 1 of subtask 1 is not reproducible: running it again with seed
0x9b1c... produced a different test on attempt 1 of 10 (they first differ at byte
0, where the original has "3\n" and the rebuilt one has "4\n"). A generator has to
take all of its randomness from the Rng it is given ...
```

Only the finished tests are rebuilt, not the candidates thrown away along the
way, and only their inputs — the solution's output follows from its input.
`Task::with_reproducibility_checks(n)` changes the count, applies the check to
file mode as well, and turns it off with `0`.

`--serve` reads one JSON object per line and answers with one per line. A served
test is byte for byte the file a normal run would have written, whitespace
included, so a judge can store seeds and materialise a test at the moment it
needs it.

```console
$ echo '{"command":"test","subtask":0,"test":0}' | ./task --serve
{"ok":true,"subtask":0,"test":0,"generator":0,"seed":"c4d89c3a3898d1aa",
 "input":"1\n990\n","output":"1\n","input_file":"test.01.001.in","output_file":"test.01.001.out"}
```

Requests:

| Request | Meaning |
| --- | --- |
| `{"command":"info"}` | The task, its subtasks and how many tests each holds. |
| `{"command":"test","subtask":0,"test":3}` | The test the manifest lists at that position. |
| `{"command":"seed","subtask":0,"generator":1,"seed":"a1b2..."}` | A test built from a generator and seed directly, whether or not the manifest lists it. |
| `{"command":"quit"}` | Stop serving. |

Add `"input":false` or `"output":false` to leave a half out; asking for the input
alone skips running the solution. Seeds are hexadecimal **strings**, because a
seed uses all 64 bits and a JSON number loses the low ones in any reader that
parses numbers as doubles.

A request that cannot be answered comes back as `{"ok":false,"error":"..."}` and
the server keeps running. Two things are refused outright, because serving them
would mean handing out the wrong test data: a manifest written for a different
task, and a generator that no longer produces the test recorded for a seed — the
manifest stores a hash of every test for exactly that check, so a task whose
generators changed is told to regenerate rather than quietly served.

## Requirements

EZCP runs on Linux, macOS and Windows. It needs:

- Rust 1.88 or newer.
- A C++ compiler, because solutions are compiled and executed:
  - **Linux** — `g++` (e.g. `apt install g++`).
  - **macOS** — `g++`/`clang++` from the Xcode command line tools (`xcode-select --install`).
  - **Windows** — a MinGW-w64 toolchain, for example via [MSYS2](https://www.msys2.org)
    (`pacman -S mingw-w64-ucrt-x86_64-gcc`) or `choco install mingw`.

The compiler is looked up on `PATH`, plus the usual install locations on Windows.
Set the `GCC_PATH` environment variable to point at a specific compiler.

Solutions get a large stack (512 MB where the platform allows it), and their time
limit is measured in CPU time, so a machine under load does not turn a correct
solution into a timeout. A solution that uses more CPU time than the limit allows
is a timeout even if it ran to completion. The running times reported in
`results.txt` are CPU time as well, so they stay comparable between runs.

## How to use this tool
For output it is not recommended to generate strings directly, but to create a struct that represents your output data and derive the `ToOutput` trait. If the output generated by the auto-derived string generator doesn't match your statement's constraints, you have two options: either modify your statement or generate raw strings.

You won't need to manually test your solutions for the most part. Correctness of your solution will always be checked if you have a naive solution that surely works and a subtask where constraints are so small that the naive solution works.

`Graph`'s constructors take the generator's `rng` as their first argument, including the ones that hold no randomness of their own (`new_empty`, `new_full`): writing a graph out shuffles its edges, and that shuffle has to come from the same seeded stream as everything else.
