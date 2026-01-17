use rand::Rng;
use std::path::PathBuf;

const SOLUTION: &str = r"
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

    let mut task = ezcp::Task::new("Coins", &PathBuf::from("task2"));
    task.solution_source = SOLUTION.to_owned();

    // Constraint: n = 1
    let mut subtask1 = ezcp::Subtask::new();



    subtask1.add_test(5, ezcp::array_generator(1, 1, 1, 1000));
    subtask1.add_test_str("1\n 1\n".to_owned());

    // Constraint: elements in the array are powers of 2 and n <= 30
    let mut subtask2 = ezcp::Subtask::new();



    subtask2.add_test(5, || {
        let mut rng = rand::rng();
        let n = rng.random_range(1..=30);
        let mut array = Vec::new();
        for i in 0..n {
            array.push(1 << i);
        }
        ezcp::array_to_string(&array, true)
    });

    // Constraint: n <= 1000
    let mut subtask3 = ezcp::Subtask::new();



    subtask3.add_test(5, ezcp::array_generator(1, 1000, 1, 1000));
    subtask3.add_test(5, ezcp::array_generator(1, 1000, 1, 1_000_000_000));
    subtask3.add_test(5, ezcp::array_generator(1, 1000, 1, 1));
    subtask3.add_test(5, ezcp::array_generator(1000, 1000, 1, 1000));
    subtask3.add_test(5, ezcp::array_generator(1000, 1000, 1, 1_000_000_000));
    subtask3.add_test(1, ezcp::array_generator(1000, 1000, 1, 1));

    // Constraint: n <= 200_000
    let mut subtask4 = ezcp::Subtask::new();



    subtask4.add_test(5, ezcp::array_generator(1, 200_000, 1, 1000));
    subtask4.add_test(5, ezcp::array_generator(1, 200_000, 1, 1_000_000_000));
    subtask4.add_test(5, ezcp::array_generator(1, 200_000, 1, 1));
    subtask4.add_test(5, ezcp::array_generator(200_000, 200_000, 1, 1000));
    subtask4.add_test(5, ezcp::array_generator(200_000, 200_000, 1, 1_000_000_000));
    subtask4.add_test(1, ezcp::array_generator(200_000, 200_000, 1, 1));

    // add subtasks to task
    let _ = task.add_subtask(subtask1);
    let _ = task.add_subtask(subtask2);
    let _ = task.add_subtask(subtask3);
    let _ = task.add_subtask(subtask4);

    // add dependencies


    task.create_tests().ok();
}
