use rand::Rng;
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

fn main() {
    // The first task you get an array of integers. You need to find the sum of all elements in the array minus the half of the maximum element.
    // Also all elements in the array are even.

    let task = ezcp::Task::new("Coupon", &PathBuf::from("task1"))
        //task.debug_level = LevelFilter::Trace;
        .with_solution_source(SOLUTION);

    // Constraint: n = 1
    // add 5 tests where an array is generated with length 1 and even values between 0 and 1_000_000_000 (inclusive)
    let subtask1 = ezcp::Subtask::new().with_test(5, ezcp::array_generator_custom(1, 1, |rng| rng.random_range(0..=500_000_000) * 2));

    // Constraint: all values are the same
    // add 5 random tests where each test is an array of length between 1 and 200_000 (inclusive) and all values are the same even value between 0 and 1_000_000_000 (inclusive)
    // add an edge case where n is maximal
    // add 3 edge cases where all values are maximal
    // add an edge case where all values and n are maximal
    let mut rng = rand::rng();
    let x = rng.random_range(0..=500_000_000) * 2;
    let subtask2 = ezcp::Subtask::new()
        .with_test(5, || {
            let mut rng = rand::rng();
            let n = rng.random_range(1..=200_000);
            let x = rng.random_range(0..=500_000_000) * 2;
            ezcp::array_to_string(&vec![x; n as usize], true)
        })
        .with_test(1, move || ezcp::array_to_string(&vec![x; 200_000], true))
        .with_test(3, ezcp::array_generator(1, 200_000, 1_000_000_000, 1_000_000_000))
        .with_test(1, || ezcp::array_to_string(&vec![1_000_000_000; 200_000], true));

    // No additional constraints
    // add some random tests
    // add 5 edge cases where n is maximal (other edge cases are handled by subtask2)
    let subtask3 = ezcp::Subtask::new()
        .with_test(5, ezcp::array_generator_custom(1, 200_000, |rng| rng.random_range(0..=500_000_000) * 2))
        .with_test(5, ezcp::array_generator_custom(200_000, 200_000, |rng| rng.random_range(0..=500_000_000) * 2));

    // add subtasks and solutions to task
    // there is a partial solution that only reads 2 integers: n, x and prints x / 2 which is correct for subtask1 but should fail for subtask2 and subtask3
    task.with_subtask(subtask1)
        .with_subtask(subtask2)
        .with_subtask(subtask3)
        .with_partial_solution(PARTIAL_SOLUTION, &[0])
        .run()
        .ok();
}
