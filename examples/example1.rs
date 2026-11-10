use ezcp::{Counted, Result, Rng, Subtask, Task, array_generator_custom};
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

fn even_value(rng: &mut Rng) -> i32 {
    rng.random_range(0..=500_000_000) * 2
}

/// `n` copies of one value. `Counted` writes `n` on one line and the values on
/// the next.
fn repeated(n: usize, value: i32) -> Counted<i32> {
    Counted(vec![value; n])
}

fn main() -> Result<()> {
    // Given an array of even integers, print its sum minus half of its maximum.
    let task = Task::new("Coupon", &PathBuf::from("task1")).with_solution_source(SOLUTION).with_memory_limit(256);

    let subtask1 = Subtask::new(10, "n = 1").with_test(5, array_generator_custom(1..=1, even_value));

    // The repeated value is drawn inside the generator: drawn out here it would
    // not come from the test's seed, so the test could not be rebuilt.
    let subtask2 = Subtask::new(20, "all values are the same")
        .with_test(5, |rng| {
            let n = rng.random_range(1..=200_000);
            repeated(n, even_value(rng))
        })
        .with_test(1, |rng| repeated(200_000, even_value(rng)))
        .with_test(3, |rng| repeated(rng.random_range(1..=200_000), 1_000_000_000))
        .with_fixed_test(repeated(200_000, 1_000_000_000));

    let subtask3 = Subtask::new(70, "No additional constraints")
        .with_test(5, array_generator_custom(1..=200_000, even_value))
        .with_test(5, array_generator_custom(200_000..=200_000, even_value))
        .with_validator(|Counted(values)| assert!(values.iter().all(|value| value % 2 == 0)));

    // The partial solution prints half of the first value, which is only right for n = 1.
    task.with_subtask(subtask1)
        .with_subtask(subtask2)
        .with_subtask(subtask3)
        .with_partial_solution("x/2", PARTIAL_SOLUTION, &[0])
        .run()
}
