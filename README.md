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
ezcp = "0.6"
```

```rust,no_run
use ezcp::{Result, Subtask, Task, ToOutput};
use std::path::PathBuf;

const SOLUTION: &str = "
#include <iostream>
int main() {
    int n; std::cin >> n;
    long long sum = 0;
    for (int i = 0; i < n; i++) { int a; std::cin >> a; sum += a; }
    std::cout << sum << std::endl;
}
";

/// The input format: the count on one line, the values on the next.
#[derive(ToOutput)]
struct Input {
    n: usize,
    values: Vec<i32>,
}

fn main() -> Result<()> {
    Task::new("Sum", &PathBuf::from("sum"))
        .with_solution_source(SOLUTION)
        .with_subtask(Subtask::new(100, "n <= 1000").with_test(10, |rng| {
            let n = rng.random_range(1..=1000);
            Input {
                n,
                values: rng.random_vec(n, 0..=1_000_000),
            }
        }))
        .run()
}
```

Running it writes `sum/tests/`, `sum/tests.zip` and `sum/results.txt`. See
`examples/` for complete tasks.

**A generator must take all of its randomness from the `rng` it is given.** A
test is identified by its generator plus its seed, so that pair has to always
rebuild the same bytes. Generators run on several threads, so they are
`Send + Sync`.

Solutions are given as source text, so that the task binary needs nothing but
itself to rebuild a test (see `--serve`); `include_str!` keeps them in files of
their own.

## Writing inputs

A generator returns anything that implements `ToOutput`, so a test is built as
the data it is rather than as text. `String` implements it too, if you would
rather write the bytes yourself.

| Value | Written as |
| --- | --- |
| a number, `char`, `bool`, `&str`, `String` | itself (`bool` as `1` or `0`) |
| `Vec`, slice, array, tuple | on one line, separated by spaces |
| `#[derive(ToOutput)]` struct | its fields in order, one per line |
| `Counted(vec)` | the length, then the `Vec` on the next line |
| `Lines(vec)` | one value per line |
| `CountedLines(vec)` | the length, then one value per line |
| `Graph` | `n m` and the edges, or one of the other `GraphFormat`s |

Values that are lines themselves, such as tuples and structs, are not joined by
spaces: `Counted(vec![(1, 2), (3, 4)])` is `2`, `1 2`, `3 4` on three lines,
which is also how several test cases go into one input.

## Generating data

`Rng` is seeded, and returns the same values on every platform and in every
version of EZCP:

| Method | Returns |
| --- | --- |
| `random_range(1..=n)` | a value from a range of integers, floats or `char`s |
| `random_range_skewed(1..=n, 3)` | the same, leaning towards the upper end (or the lower, if negative) |
| `random_vec(n, 1..=x)` | `n` such values |
| `random_interval(1..=n)` | two such values, the smaller first |
| `distinct(k, 1..=n)` | `k` different values |
| `permutation(n)`, `shuffled(items)`, `shuffle(&mut slice)` | random orders |
| `partition(total, parts, min)` | sizes that add up to `total`, as for test cases with a limit on their sum |
| `random_string(n, "abc")` | a string over an alphabet |
| `choose(&slice)`, `weighted_index(&[3, 1])` | a random element, a random index with odds |
| `random_bool(p)`, `random_f64()` | a coin toss, a value in `[0, 1)` |

`Graph` builds graphs: `new_random`, `new_random_connected`,
`new_random_bipartite`, `new_full`, `new_cycle`, `new_grid`,
`new_random_forest`, the directed `new_random_directed` and `new_random_dag`,
and trees of many shapes — `new_random_tree` (shallow), `new_random_uniform_tree`,
`new_random_deep_tree`, `new_random_path`, `new_star`, `new_caterpillar`,
`new_binary_tree`. They can be given weights (`with_random_weights`), renumbered
(`with_shuffled_labels`) and written in other layouts (`with_format`,
`zero_based`), and checked with `is_tree`, `is_connected`, `is_bipartite`,
`degree` and so on.

`array_generator(1..=n, 1..=x)` and `array_generator_custom(1..=n, |rng| ...)`
are ready-made generators of counted arrays.

## Subtasks and solutions

```rust,ignore
Subtask::new(30, "n <= 1000")
    .with_test(5, |rng| ...)              // five tests from this generator
    .with_test(0, |rng| ...)              // only used to hunt for breaking tests
    .with_fixed_test(sample)              // one hand-written test
    .with_validator(|input| assert!(...)) // panics on a test outside the constraints
    .with_stress_tests(100)               // tries each generator 100 times first
```

`Task::with_partial_solution(name, source, &[0, 2])` adds a solution that has to
pass exactly the subtasks listed (counted from zero) and fail the others. For
each subtask EZCP keeps generating until every such solution has failed
`with_min_failures` tests of it (five by default), and gives up after
`with_max_tries` candidates in a row that break nothing.

`with_checker(|input, official_output, output| ...)` replaces the comparison of
whitespace-separated tokens, for tasks with more than one correct answer.
`with_time_limit` is in milliseconds of CPU time (5000 by default);
`with_memory_limit` is in megabytes of peak memory, and there is none by default.
The results list the slowest time and the largest memory of every solution on
every subtask.

## Command line

The task binary takes the mode from its arguments (`--help` lists them):

| Arguments | What it does |
| --- | --- |
| *(none)* | Generate the tests, write them to files, archive them. |
| `--seeds` | The same, except each test file holds the seed that rebuilds the test instead of the test data. |
| `--serve` | Read such a file on stdin and write out the test data it stands for. |
| `--seed <value>` | Master seed: a number, `0x`-prefixed hex, or `random`. Ignored by `--serve`. |
| `--jobs <count>` | How many solutions run at once: by default one per CPU core, but at most 4, since solutions that compete for the machine get less reliable verdicts. The tests do not depend on it. |
| `--fail-fast` | Stop running a partial solution on a subtask it is meant to fail at its first failed test there. A solution that is too slow otherwise spends the whole time limit on each of those tests. |

`Task::with_seed`, `with_jobs` and `with_fail_fast` set the same things in code.

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

- `Graph`'s constructors take the generator's `rng`, even the ones with no
  randomness of their own — writing a graph shuffles its edges.
- Solutions get a 512 MB stack, and time limits are measured in CPU time.
- A solution is over the memory limit if its peak memory is: resident memory on
  Linux and macOS, committed memory on Windows.
- The directory the tests are written to is emptied by every run.

## Upgrading from 0.5

Seeds and stubs written by 0.5 rebuild the same tests: nothing changed in what
a generator and a seed produce. The API did change:

| 0.5 | 0.6 |
| --- | --- |
| generators and checkers of any kind | they have to be `Send + Sync` |
| `Subtask::with_test(count: i32, ..)` | `count: usize` |
| `Subtask::with_checker(fn)` | `with_validator`, which also takes closures |
| `Subtask::do_stress_test(n)` | `with_stress_tests(n)` |
| `Subtask::get_num_generators()` | `num_generators()` |
| `Task::with_get_input_file_name`, `with_get_output_file_name` | `with_input_file_name`, `with_output_file_name`, called with `usize`s |
| `Task::with_checker(fn)` | unchanged, and also takes closures |
| `Graph` sizes and counts as `i32` | `usize` |
| `Graph::get_num_nodes`, `get_num_edges`, `get_neighbours`, `get_connected_components` | `num_nodes`, `num_edges`, `neighbours` (a slice), `connected_components` |
| `Graph::edges_iter()` | `edges()` (a slice) |
| `graph.is_tree = true` | `graph.with_format(GraphFormat::Tree)` |
| `array_generator(1, n, 1, x)` returning a `String` | `array_generator(1..=n, 1..=x)` returning a `Counted` |
| a hand-written `ToOutput` | unchanged; `write_output` may be implemented too, to skip a copy |
| results: `OK (5) 12ms` | `OK (5) 12ms 3.4MB` |
