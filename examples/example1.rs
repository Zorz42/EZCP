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

/// Written as `n` on one line and the values on the next.
#[derive(ToOutput)]
struct Coupon {
    n: usize,
    values: Vec<i32>,
}

impl Coupon {
    const fn new(values: Vec<i32>) -> Self {
        Self { n: values.len(), values }
    }

    fn generate<F: Fn(&mut Rng) -> i32>(rng: &mut Rng, count: usize, value: F) -> Self {
        Self::new((0..count).map(|_ignored| value(rng)).collect())
    }
}

fn even_value(rng: &mut Rng) -> i32 {
    rng.random_range(0..=500_000_000) * 2
}

fn main() -> Result<()> {
    // Given an array of even integers, print its sum minus half of its maximum.
    let task = Task::new("Coupon", &PathBuf::from("task1")).with_solution_source(SOLUTION);

    let subtask1 = Subtask::new(10, "n = 1").with_test(5, |rng| Coupon::generate(rng, 1, even_value));

    // The repeated value is drawn inside the generator: drawn out here it would
    // not come from the test's seed, so the test could not be rebuilt.
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

    let subtask3 = Subtask::new(70, "No additional constraints")
        .with_test(5, |rng| {
            let n = rng.random_range(1..=200_000);
            Coupon::generate(rng, n, even_value)
        })
        .with_test(5, |rng| Coupon::generate(rng, 200_000, even_value));

    // The partial solution prints half of the first value, which is only right for n = 1.
    task.with_subtask(subtask1)
        .with_subtask(subtask2)
        .with_subtask(subtask3)
        .with_partial_solution("x/2", PARTIAL_SOLUTION, &[0])
        .run()
}
