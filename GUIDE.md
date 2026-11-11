# Writing tasks with EZCP

A manual for agents that build test data with EZCP. It assumes Rust and competitive programming;
`README.md` has the overview.

## What you write, what you get

A task is one Rust binary. You give it the official solution's C++ source, subtasks with points and
generators, and usually partial solutions: wrong or slow C++ solutions, each declared to pass some
subtasks and fail the rest. Running the binary writes, under the task's path:

| Path | Contents |
| --- | --- |
| `tests/test.SS.TTT.in`, `.out` | the tests: subtask `SS`, overall test `TTT`, both from 1 |
| `tests.zip` | the same files |
| `results.txt` | master seed, each solution's points and verdicts per subtask, test counts and size |
| `build/` | compiled solutions, cached by source |
| `failing_test.in`, `failing_test_correct_output.out`, `failing_test_wrong_output.out` | after the official solution or a partial solution meant to pass fails, the test it failed |

The partial solutions are how a test set is proven strong. EZCP keeps hunting for tests until each
partial solution has failed at least 5 tests of every subtask it is declared to fail, and fails the
run if one cannot be made to fail there at all. Write one for every way a contestant can go wrong
that a subtask is meant to catch: too slow, overflow, a wrong greedy, a missed edge case.

## A complete task

```rust,no_run
use ezcp::{Lines, Result, Rng, Subtask, Task, ToOutput};
use std::ops::RangeInclusive;
use std::path::Path;

// Usually `include_str!("solutions/official.cpp")`. Never read sources at run time.
const OFFICIAL: &str = r"
#include <iostream>
#include <vector>
int main() {
    int n, q; std::cin >> n >> q;
    std::vector<long long> prefix(n + 1);
    for (int i = 1; i <= n; i++) { long long a; std::cin >> a; prefix[i] = prefix[i - 1] + a; }
    while (q--) { int l, r; std::cin >> l >> r; std::cout << prefix[r] - prefix[l - 1] << '\n'; }
}
";

/// O(nq): passes the small subtasks only.
const BRUTE_FORCE: &str = r"
#include <iostream>
#include <vector>
int main() {
    int n, q; std::cin >> n >> q;
    std::vector<long long> a(n + 1);
    for (int i = 1; i <= n; i++) std::cin >> a[i];
    while (q--) { int l, r; std::cin >> l >> r; long long s = 0; for (int i = l; i <= r; i++) s += a[i]; std::cout << s << '\n'; }
}
";

/// Overflows once sums pass 2^31.
const INT_SUMS: &str = r"
#include <iostream>
#include <vector>
int main() {
    int n, q; std::cin >> n >> q;
    std::vector<int> prefix(n + 1);
    for (int i = 1; i <= n; i++) { int a; std::cin >> a; prefix[i] = prefix[i - 1] + a; }
    while (q--) { int l, r; std::cin >> l >> r; std::cout << prefix[r] - prefix[l - 1] << '\n'; }
}
";

/// `n q`, the values, then one query per line.
#[derive(Clone, ToOutput)]
struct Input {
    header: (usize, usize),
    values: Vec<i64>,
    queries: Lines<(usize, usize)>,
}

fn input(rng: &mut Rng, size: RangeInclusive<usize>, values: RangeInclusive<i64>) -> Input {
    // Skewed towards the upper end: large tests break more solutions.
    let (n, q) = (rng.random_range_skewed(size.clone(), 2), rng.random_range_skewed(size, 2));
    Input {
        header: (n, q),
        values: rng.random_vec(n, values),
        queries: Lines((0..q).map(|_| rng.random_interval(1..=n)).collect()),
    }
}

fn check(input: &Input, max_n: usize, max_value: i64) {
    let (n, q) = input.header;
    assert!((1..=max_n).contains(&n) && (1..=max_n).contains(&q) && input.values.len() == n && input.queries.0.len() == q);
    assert!(input.values.iter().all(|value| (1..=max_value).contains(value)));
    assert!(input.queries.0.iter().all(|&(l, r)| 1 <= l && l <= r && r <= n));
}

fn main() -> Result<()> {
    let sample = Input { header: (3, 2), values: vec![1, 2, 3], queries: Lines(vec![(1, 2), (2, 3)]) };

    Task::new("Range sums", Path::new("range_sums"))
        .with_solution_source(OFFICIAL)
        .with_time_limit(1000)
        .with_memory_limit(256)
        .with_subtask(
            Subtask::new(20, "n, q <= 1000, a_i <= 1000")
                .with_fixed_test(sample)
                .with_test(4, |rng| input(rng, 1..=1000, 1..=1000))
                .with_test(2, |rng| input(rng, 1000..=1000, 1000..=1000))
                .with_validator(|input: &Input| check(input, 1000, 1000)),
        )
        .with_subtask(
            Subtask::new(30, "n, q <= 1000")
                .with_test(4, |rng| input(rng, 1..=1000, 1..=1_000_000_000))
                .with_test(2, |rng| input(rng, 1000..=1000, 1_000_000_000..=1_000_000_000))
                // Only for the hunt: tests that overflow for sure.
                .with_test(0, |rng| input(rng, 900..=1000, 900_000_000..=1_000_000_000))
                .with_validator(|input: &Input| check(input, 1000, 1_000_000_000)),
        )
        .with_subtask(
            Subtask::new(50, "n, q <= 200000")
                .with_test(6, |rng| input(rng, 1..=200_000, 1..=1_000_000_000))
                .with_test(2, |rng| input(rng, 200_000..=200_000, 1..=1_000_000_000))
                .with_validator(|input: &Input| check(input, 200_000, 1_000_000_000)),
        )
        .with_partial_solution("brute force", BRUTE_FORCE, &[0, 1])
        .with_partial_solution("int sums", INT_SUMS, &[0])
        .run()
}
```

Run it with `cargo run --release` (generators of large tests are slow in debug builds), optionally
followed by `-- --fail-fast`, `--seed <n>` or `--seeds`. The binary returns an error, and so a
non-zero exit code, on any failure.

## Rules

- **All randomness comes from the generator's `rng`.** A test is stored and rebuilt as its generator
  plus a seed. Never use `rand`, the clock, `HashMap`/`HashSet` iteration order, or a random value
  drawn outside the generator and captured. Captured constants are fine. `--seeds` and
  `with_reproducibility_checks(n)` catch violations (`GeneratorNotReproducible`).
- Generators and validators are `Fn + Send + Sync + 'static`: `move` constants in, keep no mutable
  state.
- Every subtask of a task has the same input type `T`: a derived struct, a `Graph`, a `String`, etc.
- `with_partial_solution` indices count subtasks from **zero**; numbers in messages and file names
  count from one.
- `with_fixed_test` needs `T: Clone`. `Graph` is not `Clone`; write a fixed graph as `with_test(1, ...)`.
- Validators assert the subtask's constraints. A panic in a generator or validator ends the run with
  its message, so validate every subtask.
- Inputs and official outputs have their whitespace normalised (runs collapse, one trailing newline)
  unless `trim_whitespace(false)`.
- Time limits are CPU milliseconds, memory limits megabytes of peak memory.

## Writing inputs

| Value | Written as |
| --- | --- |
| number, `char`, `bool`, `&str`, `String` | itself (`bool` as `1`/`0`) |
| `Vec`, slice, array, tuple | one line, space-separated |
| `#[derive(ToOutput)]` struct | each field on its own line, in order; empty fields skipped |
| `Counted(vec)` | length line, then the `Vec` |
| `Lines(vec)` | one value per line |
| `CountedLines(vec)` | length line, then one value per line |
| `Graph` | per its `GraphFormat` |

A value that already ends a line (tuple, struct, `Lines`, `Graph`) is not followed by a space, so a
`Vec` of them is one per line. Several test cases in one input:

```rust,ignore
#[derive(ToOutput)]
struct Case { n: usize, values: Vec<i64> }

// t cases whose n add up to 200000.
let sizes = rng.partition(200_000_usize, t, 1);
Counted(sizes.into_iter().map(|n| Case { n, values: rng.random_vec(n, 1..=1_000_000_000) }).collect())
```

A hand-written impl writes into the buffer: `impl ToOutput for X { fn write_output(self, out: &mut String) { ... } }`.

## Rng

`random_range(a..=b)` (integers, floats, `char`), `random_range_skewed(range, k)` (max of `k + 1`
draws; negative `k`: min of `1 - k`), `random_vec(n, range)`, `random_interval(range)` (`l <= r`),
`distinct(k, range)`, `permutation(n)` (of `1..=n`), `shuffled(iter)`, `shuffle(&mut slice)`,
`partition(total, parts, min_part)`, `random_string(n, alphabet)`, `choose(&slice)`,
`weighted_index(&[w0, w1, ...])`, `random_bool(p)`, `random_f64()`, `next_u64()`.

## Graph

Nodes are `0..n` in code and written from 1 (`zero_based()` to change that). Edges are written in
random order with random end order (directed edges keep their direction).

- Trees (format `Tree`: `n`, then `n - 1` edges): `new_random_tree` (about `ln n` deep),
  `new_random_uniform_tree` (`sqrt n`), `new_random_deep_tree` (`n / 2`), `new_random_path`,
  `new_star`, `new_caterpillar(rng, n, spine)`, `new_binary_tree`.
- Graphs (format `EdgeList`: `n m`, then the edges): `new_empty`, `new_full`, `new_random(rng, n, m)`,
  `new_random_connected(rng, n, m)`, `new_random_bipartite(rng, n, m)`, `new_random_forest(rng, n,
  trees)`, `new_cycle`, `new_grid(rng, rows, columns)`, and directed `new_empty_directed`,
  `new_random_directed`, `new_random_dag`.
- `with_format(GraphFormat::ParentList)`: `n`, then the parents of nodes 2..n rooted at 1, and the
  weights on a third line if any. `GraphFormat::EdgesOnly`: the edges alone, for a custom header.
- `with_random_weights(rng, 1..=x)` / `with_weights(vec)` add a weight after each edge; add every
  edge first. `with_shuffled_labels(rng)` hides structure left in the numbering (grids, paths).
- `add_edge(u, v)` (ignores loops and duplicates), `has_edge`, `edges()`, `neighbours`, `degree`,
  `num_nodes`, `num_edges`, `is_tree`, `is_connected`, `is_bipartite`, `connected_components`.

Mix tree shapes in one generator: a solution is often slow or wrong on only one of them.

```rust,ignore
match rng.weighted_index(&[3, 2, 2, 1, 1]) {
    0 => Graph::new_random_uniform_tree(rng, n),
    1 => Graph::new_random_deep_tree(rng, n),
    2 => Graph::new_random_path(rng, n),
    3 => Graph::new_star(rng, n),
    _ => Graph::new_caterpillar(rng, n, n.div_ceil(2)),
}
```

## Designing a test set

- Per subtask, cover: random sizes skewed large; the maximum size exactly; the minimum (`n = 1`);
  extreme values (all maximal, all equal, sorted, alternating); structures that break naive or
  recursive solutions (paths, stars, deep trees). Validators keep each within the subtask.
- A few initial tests per generator (2–6) is enough; the hunt adds what the partial solutions need.
- Targeted generators that are only for the hunt get count 0, so they add tests only when those tests
  break something.
- Put the statement's samples first, with `with_fixed_test`, in the first subtask they satisfy.
- `with_stress_tests(n)` on a subtask runs each generator `n` times through the solutions meant to
  pass before generating, which catches a generator that breaks the constraints or the solutions.
- `with_min_failures(n)` and `with_max_tries(n)`, on the task or a subtask, tune how hard the hunt
  tries (5 and 100).
- Slow partial solutions spend the whole time limit on each test they are meant to fail; `--fail-fast`
  stops them at the first failure of such a subtask in the final run.
- Several correct answers: `with_checker(|input, official_output, output| ...)`, which must accept the
  official output itself.

## When a run fails

| Error | Meaning and fix |
| --- | --- |
| `CompilerError` | A C++ source did not compile; the message has the compiler's output. |
| `SolutionTimedOut`, `SolutionOutOfMemory`, `SolutionCrash` | The official solution failed on a test (`failing_test.in`). Either it is wrong or slow, or the generator breaks the constraints: add or tighten the validator. |
| `SolutionFailed` | The checker rejected the official solution's own output. |
| `PartialSolutionFailsSubtask` | A partial solution failed a subtask it is declared to pass (verdict and generator in the message, test in the `failing_test*` files). Fix the solution, its subtask list, or the generator. |
| `PartialSolutionPassesExtraSubtask` | No test of a subtask breaks a partial solution meant to fail it. Add a generator aimed at its weakness (count 0 if it should only hunt), raise `with_max_tries`, or fix the subtask list. |
| `InvalidSubtaskIndex` | A subtask index counted from one. |
| `GeneratorNotReproducible` | A generator uses randomness other than its `rng`. |
| `TestAlreadyExists` | Custom file names collide. |

Warnings that do not fail the run but weaken the tests: "only has X of the Y tests that partial
solution ... is supposed to fail" (the hunt ran out of tries) and "produced only k of n tests" (a
generator with too small a range keeps repeating itself).

## Reading `results.txt`

```text
Master seed: 0x455a435053454544
Running official solution:
Points 100/100
Lines of code: 18
Results:
- Subtask 1: OK (5) 0ms 1.8MB
- Subtask 2: OK (10) 114ms 1.7MB
Running partial solution 1: x/2
Points 10/100
Lines of code: 12
Results:
- Subtask 1: OK (5) 0ms 1.7MB
- Subtask 2: WA (10)
Tests size: 23.74MB
Subtask 1: 5 tests
Subtask 2: 10 tests
```

Verdict counts per subtask (`OK`, `WA`, `TLE`, `MLE`, `RTE`, `skipped` with `--fail-fast`), then the
slowest time and largest memory if all passed. Check that the official solution's slowest time is
well below the limit, and that every partial solution scores what it should.

## Checklist

1. The official solution is correct and fast.
2. Every subtask has a validator and tests at its maximum size.
3. Every intended mistake has a partial solution with the right 0-based subtask list.
4. The run succeeds without "only has X of the Y tests" warnings.
5. `results.txt` shows the official solution comfortably within the limits.
6. `--seeds` succeeds too, if the tests will be stored as seeds.
