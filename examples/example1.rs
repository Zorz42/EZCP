use ezcp::{Result, Rng, Subtask, Task, ToOutput};
use std::path::PathBuf;

const SOLUTION: &str = r#"
#include<iostream>
using namespace std;

int main(){
    int n;
    cin>>n;
    long long sum=0;
    int big=0;
    while(n--){
        int a;
        cin>>a;
        big=max(big,a);
        sum+=a;
    }
    cout<<sum-big/2<<"\n";
}
"#;

const PARTIAL_SOLUTION: &str = r#"
#include<iostream>
using namespace std;

int main(){
    int n;
    cin>>n;
    int x;
    cin>>x;
    cout<<x/2<<"\n";
}
"#;

/// One test's input: the length on the first line, the values on the second.
///
/// Deriving `ToOutput` writes the fields out in declaration order, one line
/// each, and a `Vec` puts its values on one line separated by spaces - which is
/// this format exactly. A generator can then build the data the task is about
/// and never assemble a string.
#[derive(ToOutput)]
struct Coupon {
    n: usize,
    values: Vec<i32>,
}

impl Coupon {
    /// The count is whatever the array holds, so it is taken from the values
    /// rather than passed in beside them and kept in step by hand.
    const fn new(values: Vec<i32>) -> Self {
        Self { n: values.len(), values }
    }

    /// An array of `count` values, each one drawn by `value`.
    fn generate<F: Fn(&mut Rng) -> i32>(rng: &mut Rng, count: usize, value: F) -> Self {
        Self::new((0..count).map(|_ignored| value(rng)).collect())
    }
}

/// An even value in range, drawn from the seeded generator it is handed.
fn even_value(rng: &mut Rng) -> i32 {
    rng.random_range(0..=500_000_000) * 2
}

fn main() -> Result<()> {
    // The first task you get an array of integers. You need to find the sum of all elements in the array minus the half of the maximum element.
    // Also all elements in the array are even.

    let task = Task::new("Coupon", &PathBuf::from("task1"))
        //task.debug_level = LevelFilter::Trace;
        .with_solution_source(SOLUTION);

    // Constraint: n = 1
    // add 5 tests where an array is generated with length 1 and even values between 0 and 1_000_000_000 (inclusive)
    let subtask1 = Subtask::new(10, "n = 1").with_test(5, |rng| Coupon::generate(rng, 1, even_value));

    // Constraint: all values are the same
    // add 5 random tests where each test is an array of length between 1 and 200_000 (inclusive) and all values are the same even value between 0 and 1_000_000_000 (inclusive)
    // add an edge case where n is maximal
    // add 3 edge cases where all values are maximal
    // add an edge case where all values and n are maximal
    // Note that the repeated value is drawn inside the generator, from the seeded
    // `rng` it is handed. Drawing it out here, when the task is being described,
    // would bake one value into the binary and make the test impossible to
    // reproduce from its seed.
    let subtask2 = Subtask::new(20, "all values are the same")
        .with_test(5, |rng| {
            let n = rng.random_range(1..=200_000);
            Coupon::new(vec![even_value(rng); n])
        })
        .with_test(1, |rng| Coupon::new(vec![even_value(rng); 200_000]))
        .with_test(3, |rng| {
            let n = rng.random_range(1..=200_000);
            Coupon::new(vec![1_000_000_000; n])
        })
        .with_test(1, |_rng| Coupon::new(vec![1_000_000_000; 200_000]));

    // No additional constraints
    // add some random tests
    // add 5 edge cases where n is maximal (other edge cases are handled by subtask2)
    let subtask3 = Subtask::new(70, "No additional constraints")
        .with_test(5, |rng| {
            let n = rng.random_range(1..=200_000);
            Coupon::generate(rng, n, even_value)
        })
        .with_test(5, |rng| Coupon::generate(rng, 200_000, even_value));

    // add subtasks and solutions to task
    // there is a partial solution that only reads 2 integers: n, x and prints x / 2 which is correct for subtask1 but should fail for subtask2 and subtask3
    task.with_subtask(subtask1)
        .with_subtask(subtask2)
        .with_subtask(subtask3)
        .with_partial_solution("x/2", PARTIAL_SOLUTION, &[0])
        .run()
}
