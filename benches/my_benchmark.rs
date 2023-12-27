use criterion::{criterion_group, criterion_main, Criterion};
use ezcp::{array_generator, Task};
use std::path::PathBuf;

const TESTS_DIR: &str = "bench_tasks/";

fn perf_one_test(iters: i32) {
    let task_name = format!("one_test_one_subtask_{iters}");
    let task_path = PathBuf::from(TESTS_DIR).join(&task_name);

    let mut task = Task::new(&task_name, &task_path);

    // create directory
    std::fs::create_dir_all(task_path.clone()).unwrap();

    if !task_path.join("solution.cpp").exists() {
        // create solution file
        let solution_contents = r#"
        #include <iostream>
        
        int main() {
            std::cout<<"1\n";
            return 0; 
        }
        
        "#;

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();
    }

    let mut subtask1 = ezcp::Subtask::new(100);
    subtask1.add_test_str("1\n");

    // create subtasks
    task.add_subtask(subtask1);

    for _ in 0..iters {
        assert!(task.create_tests_no_print());
    }
}

fn perf_arrays(num_tests: i32, min_n: i32, max_n: i32, min_x: i32, max_x: i32) {
    let task_name = "arrays";
    let task_path = PathBuf::from(TESTS_DIR).join(task_name);

    let mut task = Task::new(task_name, &task_path);

    // create directory
    std::fs::create_dir_all(task_path.clone()).unwrap();

    if !task_path.join("solution.cpp").exists() {
        // create solution file
        let solution_contents = r#"
        #include <iostream>
        
        int main() {
            std::cout<<"1\n";
            return 0; 
        }
        
        "#;

        std::fs::write(task_path.join("solution.cpp"), solution_contents).unwrap();
    }

    let mut subtask1 = ezcp::Subtask::new(100);
    subtask1.add_test(num_tests, array_generator(min_n, max_n, min_x, max_x));

    // create subtasks
    task.add_subtask(subtask1);

    assert!(task.create_tests_no_print());
}

fn criterion_benchmark(c: &mut Criterion) {
    std::fs::remove_dir_all(TESTS_DIR).unwrap_or(());

    c.bench_function("one test", |b| b.iter(|| perf_one_test(1)));
    c.bench_function("one test reran 20 times", |b| b.iter(|| perf_one_test(20)));
    c.bench_function("small arrays", |b| b.iter(|| perf_arrays(50, 1, 100, 1, 100)));
    c.bench_function("big arrays", |b| b.iter(|| perf_arrays(50, 1, 1_000_000, 1, 1_000_000_000)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
