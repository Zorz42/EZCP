use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ezcp::Task;
use std::path::PathBuf;

const TESTS_DIR: &str = "bench_tasks/";

fn perf_one_test_one_subtask(iters: i32) {
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

fn criterion_benchmark(c: &mut Criterion) {
    std::fs::remove_dir_all(TESTS_DIR).unwrap_or(());

    c.bench_function("one test, one subtask", |b| b.iter(|| perf_one_test_one_subtask(1)));
    c.bench_function("one test, one subtask 100 times", |b| b.iter(|| perf_one_test_one_subtask(100)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
