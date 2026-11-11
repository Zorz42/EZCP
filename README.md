# EZCP

Test data generation for competitive programming tasks.

You describe a task: an official C++ solution, subtasks with generators, and optionally partial
solutions and a checker. EZCP generates the inputs, runs the official solution for the outputs, keeps
generating until every partial solution fails the subtasks it should, and writes the tests and a zip
archive. A test is its generator plus a seed, so the same code always produces the same tests.

[GUIDE.md](https://github.com/Zorz42/EZCP/blob/main/GUIDE.md) is a compact manual for writing tasks,
meant for agents.

## Requirements

Rust 1.88+ and a C++ compiler on `PATH` (`g++` or `clang++`; MinGW-w64 on Windows), or one named by
`GCC_PATH`.

## Example

```toml
[dependencies]
ezcp = "0.6"
```

```rust,no_run
use ezcp::{Result, Subtask, Task, ToOutput};
use std::path::Path;

const SOLUTION: &str = "
#include <iostream>
int main() {
    int n; std::cin >> n;
    long long sum = 0;
    for (int i = 0; i < n; i++) { int a; std::cin >> a; sum += a; }
    std::cout << sum << std::endl;
}
";

/// The count on one line, the values on the next.
#[derive(ToOutput)]
struct Input {
    n: usize,
    values: Vec<i32>,
}

fn main() -> Result<()> {
    Task::new("Sum", Path::new("sum"))
        .with_solution_source(SOLUTION)
        .with_subtask(Subtask::new(100, "n <= 1000").with_test(10, |rng| {
            let n = rng.random_range(1..=1000);
            Input { n, values: rng.random_vec(n, 0..=1_000_000) }
        }))
        .run()
}
```

`cargo run --release` writes `sum/tests/`, `sum/tests.zip` and `sum/results.txt`. See `examples/`
for complete tasks.

A generator must take all of its randomness from its `rng`, since a test is rebuilt from its
generator and seed, and be `Send + Sync`, since generators run on several threads. Solutions are
source text (`include_str!` keeps them in files of their own), so the task binary needs nothing but
itself.

## How a run goes

For each subtask: the stress tests, if asked for; the initial tests of every generator; then the
hunt, which draws candidates from the subtask's generators and keeps those that break a partial
solution meant to fail the subtask, until each such solution has failed `with_min_failures` tests (5)
or `with_max_tries` candidates in a row (100) broke nothing. A partial solution meant to pass that
fails, or one meant to fail that fails nothing, is an error. Every solution then runs on the finished
tests for `results.txt`, and only then are the tests written.

## Inputs

A generator returns anything that implements `ToOutput`:

| Value | Written as |
| --- | --- |
| a number, `char`, `bool`, `&str`, `String` | itself (`bool` as `1` or `0`) |
| `Vec`, slice, array, tuple | on one line, separated by spaces |
| `#[derive(ToOutput)]` struct | its fields in order, one per line |
| `Counted(vec)` | the length, then the `Vec` on the next line |
| `Lines(vec)` | one value per line |
| `CountedLines(vec)` | the length, then one value per line |
| `Graph` | `n m` and the edges, or another `GraphFormat` |

Values that are lines themselves, such as tuples and structs, are not joined by spaces:
`Counted(vec![(1, 2), (3, 4)])` is `2`, `1 2`, `3 4` on three lines, which is also how several test
cases go into one input. A hand-written `ToOutput` implements `write_output`.

## Randomness

`Rng` returns the same values on every platform and in every version:

| Method | Returns |
| --- | --- |
| `random_range(1..=n)` | a value from a range of integers, floats or `char`s |
| `random_range_skewed(1..=n, 3)` | the same, leaning to the upper end (the lower if negative) |
| `random_vec(n, 1..=x)` | `n` such values |
| `random_interval(1..=n)` | two such values, the smaller first |
| `distinct(k, 1..=n)` | `k` different values |
| `permutation(n)`, `shuffled(items)`, `shuffle(&mut slice)` | random orders (`permutation` of `1..=n`) |
| `partition(total, parts, min)` | sizes adding up to `total`, as for a limit on the sum of `n` |
| `random_string(n, "abc")` | a string over an alphabet |
| `choose(&slice)`, `weighted_index(&[3, 1])` | a random element, an index with odds |
| `random_bool(p)`, `random_f64()` | a coin toss, a value in `[0, 1)` |

`Graph` builds graphs: `new_random`, `new_random_connected`, `new_random_bipartite`, `new_full`,
`new_cycle`, `new_grid`, `new_random_forest`, the directed `new_random_directed` and `new_random_dag`,
and trees: `new_random_tree` (shallow), `new_random_uniform_tree`, `new_random_deep_tree`,
`new_random_path`, `new_star`, `new_caterpillar`, `new_binary_tree`. It takes weights
(`with_random_weights`), new labels (`with_shuffled_labels`) and other layouts (`with_format`,
`zero_based`), and answers `is_tree`, `is_connected`, `is_bipartite`, `degree` and so on.

`array_generator(1..=n, 1..=x)` and `array_generator_custom(1..=n, |rng| ...)` are ready-made
generators of counted arrays.

## Subtasks and solutions

```rust,ignore
Subtask::new(30, "n <= 1000")
    .with_test(5, |rng| ...)              // five tests from this generator
    .with_test(0, |rng| ...)              // only used by the hunt
    .with_fixed_test(sample)              // one hand-written test
    .with_validator(|input| assert!(...)) // panics on a test outside the constraints
    .with_stress_tests(100)               // runs each generator 100 times first
```

`Task::with_partial_solution(name, source, &[0, 2])` adds a solution that has to pass exactly the
subtasks listed, counted from zero, and fail the others. `with_checker(|input, official_output,
output| ...)` replaces the comparison of whitespace-separated tokens. `with_time_limit` is in
milliseconds of CPU time (5000); `with_memory_limit` in megabytes of peak memory (none). The results
list each solution's slowest time and largest memory on every subtask.

## Command line

| Arguments | What it does |
| --- | --- |
| *(none)* | Generate the tests, write them, archive them. |
| `--seeds` | The same, but each test file holds the seed that rebuilds it instead of the data. |
| `--serve` | Read such files on stdin, one per line, and write out the data each stands for. |
| `--seed <value>` | Master seed: a number, `0x`-prefixed hex, or `random`. |
| `--jobs <count>` | Solutions run at once: one per CPU core, at most 4. The tests do not depend on it. |
| `--fail-fast` | In the final run, stop a partial solution on a subtask it should fail at its first failure there, instead of spending the time limit on every test. |

`with_seed`, `with_jobs` and `with_fail_fast` set the same in code.

With `--seeds`, everything is generated and verified as usual, and the file names and archive are the
same; only the data is left out, so `examples/example2` takes 12 kB instead of 22 MB. Each kept test is
first rebuilt from its seed ten times (`with_reproducibility_checks`), so a generator that is not
faithful to its `Rng` fails the run instead of leaving a seed behind that does not rebuild its test.

```console
$ ./task --serve < tests/test.01.001.in > test.in
$ ./task --serve < tests/test.01.001.out > test.out
```

The answer is byte for byte what a normal run writes. Rebuilding an output runs the official solution.
A file that cannot be rebuilt, such as one whose generator has changed since, produces no output: the
error goes to stderr and the exit code is non-zero.

## Notes

- Solutions get a 512 MB stack.
- Peak memory is resident memory on Linux and macOS, committed memory on Windows.
- On Linux, solutions are linked statically if the compiler has `libc.a` and `libstdc++.a` (on Fedora
  `glibc-static` and `libstdc++-static`), which halves the cost of a short run.
- The tests directory is emptied by every run.
