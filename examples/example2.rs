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

    sort(a, a + n);

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

/// Written as `n` on one line and the values on the next.
#[derive(ToOutput)]
struct Coins {
    n: usize,
    values: Vec<i32>,
}

impl Coins {
    const fn new(values: Vec<i32>) -> Self {
        Self { n: values.len(), values }
    }

    fn random(rng: &mut Rng, n: RangeInclusive<i32>, x: RangeInclusive<i32>) -> Self {
        let count = rng.random_range(n);
        Self::new((0..count).map(|_ignored| rng.random_range(x.clone())).collect())
    }
}

fn main() -> Result<()> {
    // Given n coins, print the smallest sum they cannot make (8 for coins 1, 2, 4).
    let task = Task::new("Coins", &PathBuf::from("task2")).with_solution_source(SOLUTION);

    let subtask1 = Subtask::new(10, "n = 1")
        .with_test(5, |rng| Coins::random(rng, 1..=1, 1..=1000))
        .with_test(1, |_rng| Coins::new(vec![1]));

    let subtask2 = Subtask::new(20, "elements in the array are powers of 2 and n <= 30").with_test(5, |rng| {
        let n = rng.random_range(1..=30);
        Coins::new((0..n).map(|i| 1 << i).collect())
    });

    let subtask3 = Subtask::new(30, "n <= 1000")
        .with_test(5, |rng| Coins::random(rng, 1..=1000, 1..=1000))
        .with_test(5, |rng| Coins::random(rng, 1..=1000, 1..=1_000_000_000))
        .with_test(5, |rng| Coins::random(rng, 1..=1000, 1..=1))
        .with_test(5, |rng| Coins::random(rng, 1000..=1000, 1..=1000))
        .with_test(5, |rng| Coins::random(rng, 1000..=1000, 1..=1_000_000_000))
        .with_test(1, |rng| Coins::random(rng, 1000..=1000, 1..=1));

    let subtask4 = Subtask::new(40, "n <= 200_000")
        .with_test(5, |rng| Coins::random(rng, 1..=200_000, 1..=1000))
        .with_test(5, |rng| Coins::random(rng, 1..=200_000, 1..=1_000_000_000))
        .with_test(5, |rng| Coins::random(rng, 1..=200_000, 1..=1))
        .with_test(5, |rng| Coins::random(rng, 200_000..=200_000, 1..=1000))
        .with_test(5, |rng| Coins::random(rng, 200_000..=200_000, 1..=1_000_000_000))
        .with_test(1, |rng| Coins::random(rng, 200_000..=200_000, 1..=1));

    task.with_subtask(subtask1).with_subtask(subtask2).with_subtask(subtask3).with_subtask(subtask4).run()
}
