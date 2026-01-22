use rand::Rng;
use std::path::PathBuf;

const SOLUTION: &str = "
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

fn main() {
    // In this task you have n coins with values a1, a2, ..., an. You need to find the smallest sum, you cannot get using these coins.
    // For example, if you have coins with values 1, 2 and 4, you can get any sum from 1 to 7, but you cannot get 8.

    let task = ezcp::Task::new("Coins", &PathBuf::from("task2")).with_solution_source(SOLUTION.to_owned());

    // Constraint: n = 1
    let subtask1 = ezcp::Subtask::new().with_test(5, ezcp::array_generator(1, 1, 1, 1000)).with_test_str("1\n 1\n");

    // Constraint: elements in the array are powers of 2 and n <= 30
    let subtask2 = ezcp::Subtask::new().with_test(5, || {
        let mut rng = rand::rng();
        let n = rng.random_range(1..=30);
        let mut array = Vec::new();
        for i in 0..n {
            array.push(1 << i);
        }
        ezcp::array_to_string(&array, true)
    });

    // Constraint: n <= 1000
    let subtask3 = ezcp::Subtask::new()
        .with_test(5, ezcp::array_generator(1, 1000, 1, 1000))
        .with_test(5, ezcp::array_generator(1, 1000, 1, 1_000_000_000))
        .with_test(5, ezcp::array_generator(1, 1000, 1, 1))
        .with_test(5, ezcp::array_generator(1000, 1000, 1, 1000))
        .with_test(5, ezcp::array_generator(1000, 1000, 1, 1_000_000_000))
        .with_test(1, ezcp::array_generator(1000, 1000, 1, 1));

    // Constraint: n <= 200_000
    let subtask4 = ezcp::Subtask::new()
        .with_test(5, ezcp::array_generator(1, 200_000, 1, 1000))
        .with_test(5, ezcp::array_generator(1, 200_000, 1, 1_000_000_000))
        .with_test(5, ezcp::array_generator(1, 200_000, 1, 1))
        .with_test(5, ezcp::array_generator(200_000, 200_000, 1, 1000))
        .with_test(5, ezcp::array_generator(200_000, 200_000, 1, 1_000_000_000))
        .with_test(1, ezcp::array_generator(200_000, 200_000, 1, 1));

    // add subtasks to task
    task.with_subtask(subtask1).with_subtask(subtask2).with_subtask(subtask3).with_subtask(subtask4).run().ok();
}
