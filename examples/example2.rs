use ezcp::{Result, Rng, Subtask, Task, ToOutput};
use std::ops::RangeInclusive;
use std::path::PathBuf;

const SOLUTION: &str = "
#include <algorithm>
#include <iostream>
using namespace std;
int main() {
    int n;
    cin >> n;
    int a[n];
    for (int i = 0; i < n; i++) {
        cin >> a[i];
    }

    // Sort the array
    sort(a, a + n);

    // Find the smallest sum that cannot be formed
    int smallest_sum = 1;
    for (int i = 0; i < n; i++) {
        if (a[i] > smallest_sum) {
            break;
        }
        smallest_sum += a[i];
    }

    cout << smallest_sum << endl;
    return 0;
}
";

/// One test's input: how many coins there are, then their values.
///
/// Deriving `ToOutput` writes the fields out in declaration order, one line
/// each, and a `Vec` puts its values on one line separated by spaces - which is
/// this format exactly. A generator can then build the data the task is about
/// and never assemble a string.
#[derive(ToOutput)]
struct Coins {
    n: usize,
    values: Vec<i32>,
}

impl Coins {
    /// The count is whatever the array holds, so it is taken from the values
    /// rather than passed in beside them and kept in step by hand.
    const fn new(values: Vec<i32>) -> Self {
        Self { n: values.len(), values }
    }

    /// A random test: as many coins as `n` allows, each worth something `x`
    /// allows. Both are drawn from the seeded generator, so the same seed always
    /// gives the same coins.
    fn random(rng: &mut Rng, n: RangeInclusive<i32>, x: RangeInclusive<i32>) -> Self {
        let count = rng.random_range(n);
        let (min, max) = (*x.start(), *x.end());
        Self::new((0..count).map(|_ignored| rng.random_range(min..=max)).collect())
    }
}

fn main() -> Result<()> {
    // In this task you have n coins with values a1, a2, ..., an. You need to find the smallest sum, you cannot get using these coins.
    // For example, if you have coins with values 1, 2 and 4, you can get any sum from 1 to 7, but you cannot get 8.

    let task = Task::new("Coins", &PathBuf::from("task2")).with_solution_source(SOLUTION);

    // Constraint: n = 1
    let subtask1 = Subtask::new(10, "n = 1")
        .with_test(5, |rng| Coins::random(rng, 1..=1, 1..=1000))
        .with_test(1, |_rng| Coins::new(vec![1]));

    // Constraint: elements in the array are powers of 2 and n <= 30
    let subtask2 = Subtask::new(20, "elements in the array are powers of 2 and n <= 30").with_test(5, |rng| {
        let n = rng.random_range(1..=30);
        Coins::new((0..n).map(|i| 1 << i).collect())
    });

    // Constraint: n <= 1000
    let subtask3 = Subtask::new(30, "n <= 1000")
        .with_test(5, |rng| Coins::random(rng, 1..=1000, 1..=1000))
        .with_test(5, |rng| Coins::random(rng, 1..=1000, 1..=1_000_000_000))
        .with_test(5, |rng| Coins::random(rng, 1..=1000, 1..=1))
        .with_test(5, |rng| Coins::random(rng, 1000..=1000, 1..=1000))
        .with_test(5, |rng| Coins::random(rng, 1000..=1000, 1..=1_000_000_000))
        .with_test(1, |rng| Coins::random(rng, 1000..=1000, 1..=1));

    // Constraint: n <= 200_000
    let subtask4 = Subtask::new(40, "n <= 200_000")
        .with_test(5, |rng| Coins::random(rng, 1..=200_000, 1..=1000))
        .with_test(5, |rng| Coins::random(rng, 1..=200_000, 1..=1_000_000_000))
        .with_test(5, |rng| Coins::random(rng, 1..=200_000, 1..=1))
        .with_test(5, |rng| Coins::random(rng, 200_000..=200_000, 1..=1000))
        .with_test(5, |rng| Coins::random(rng, 200_000..=200_000, 1..=1_000_000_000))
        .with_test(1, |rng| Coins::random(rng, 200_000..=200_000, 1..=1));

    // add subtasks to task
    task.with_subtask(subtask1).with_subtask(subtask2).with_subtask(subtask3).with_subtask(subtask4).run()
}
