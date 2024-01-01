# EZCP
A rust framework to easily create tasks for competitive programming.

Features:
- Generate tests and save them into files.
- Generate solutions.
- Optionally have a testcase checker.
- Make a solution checker if there are multiple valid solutions.
- Have subtask dependencies.
- Have a premade graph generator.
- Have a premade array generator.
- More common and useful premade generators.
- Automatically generate some parts of the statement (time/memory limit, subtasks, samples...) and save it to latex.
- Add partial solutions and specify which subtasks it should pass.

Suggestions and bug reports: jakob@zorz.si
You can also open a pull request.

Minimal example: (see examples/ for more complete examples)
```rust
use rand::Rng;
use std::path::PathBuf;

fn main() {
    // The first task you get an array of integers. You need to find the sum of all elements in the array minus the half of the maximum element.
    // Also all elements in the array are even.

    let mut task = ezcp::Task::new("Coupon", &PathBuf::from("coupon"));

    // Constraint: n = 1
    let mut subtask1 = ezcp::Subtask::new(20);
    
    // add 5 tests where an array is generated with length 1 and even values between 0 and 1_000_000_000 (inclusive)
    subtask1.add_test(5, ezcp::array_generator_custom(1, 1, |rng| rng.gen_range(0..=500_000_000) * 2));
    

    // No additional constraints
    let mut subtask2 = ezcp::Subtask::new(50);

    // add some random tests
    subtask2.add_test(5, ezcp::array_generator_custom(1, 200_000, |rng| rng.gen_range(0..=500_000_000) * 2));

    // add 5 edge cases where n is maximal
    subtask2.add_test(5, ezcp::array_generator_custom(200_000, 200_000, |rng| rng.gen_range(0..=500_000_000) * 2));

    // add subtasks to task
    let subtask1 = task.add_subtask(subtask1);
    let subtask2 = task.add_subtask(subtask2);

    // add dependencies (dependencies are only if constraints of a dependency are a subset of constraints of a subtask)
    task.add_subtask_dependency(subtask2, subtask1);
    
    // finally create the tests
    task.create_tests();
}
```

And `coupon/solution.cpp`:
```cpp
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
```
