# EZCP
A Rust framework to easily create tasks for competitive programming.

Features:
- Generate test inputs and save them into files.
- Generate correct outputs.
- Optionally have a test input checker.
- [TODO] Make a solution checker if there are multiple valid solutions.
- Subtask dependencies.
- Graph generator.
- Array generator.
- Add a partial solution and specify which subtasks it should pass.
- Automatically archive all test files into a zip.
- Support for CPS https://github.com/Zorz42/CPS

Suggestions and bug reports: jakob@zorz.si
You can also open a pull request.

Minimal example: (see `examples/` for more complete examples)
```rust
use rand::Rng;
use std::path::PathBuf;

fn main() {
    // For the first task you get an array of even integers. 
    // You need to find the sum of all elements in the array minus the half of the maximum element.

    let mut task = ezcp::Task::new("Coupon", &PathBuf::from("coupon"));

    // Constraint: n = 1
    let mut subtask1 = ezcp::Subtask::new(20);
    
    // Add 5 tests, where an array is generated with length 1 and an even value between 0 and 1_000_000_000 (inclusive).
    subtask1.add_test(5, ezcp::array_generator_custom(1, 1, |rng| rng.gen_range(0..=500_000_000) * 2));
    

    // No additional constraints.
    let mut subtask2 = ezcp::Subtask::new(50);

    // Add some random tests.
    subtask2.add_test(5, ezcp::array_generator_custom(1, 200_000, |rng| rng.gen_range(0..=500_000_000) * 2));

    // Add 5 edge cases, where n is maximal.
    subtask2.add_test(5, ezcp::array_generator_custom(200_000, 200_000, |rng| rng.gen_range(0..=500_000_000) * 2));

    // Add subtasks to the task.
    let subtask1 = task.add_subtask(subtask1);
    let subtask2 = task.add_subtask(subtask2);

    // Add dependencies (dependencies are only if constraints of a dependency are a subset of constraints of a subtask).
    task.add_subtask_dependency(subtask2, subtask1);
    
    // Finally, create the tests.
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
