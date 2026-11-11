use crate::rng::Rng;
use crate::task::DEFAULT_SEED;
use crate::tests::{DOUBLE, test_files, test_task, wrong_when};
use crate::{Error, Mode, Subtask, Task};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn number(range: std::ops::RangeInclusive<i64>) -> impl Fn(&mut Rng) -> String + Send + Sync {
    move |rng| format!("{}\n", rng.random_range(range.clone()))
}

/// The lines of the results file about `solution`'s subtasks.
fn results_of(path: &Path, solution: &str) -> Vec<String> {
    let results = std::fs::read_to_string(path.join("results.txt")).unwrap();
    let after = results.split_once(solution).unwrap_or_else(|| unreachable!("no {solution} in {results}")).1;
    let is_subtask = |line: &&str| line.starts_with("- Subtask");
    after.lines().skip_while(|line| !is_subtask(line)).take_while(is_subtask).map(str::to_owned).collect()
}

/// The partial solution passes the first subtask and fails most of the second.
fn two_subtasks(task: Task<String>) -> Task<String> {
    task.with_solution_source(DOUBLE)
        .with_subtask(Subtask::new(40, "small").with_test(6, number(1..=100)))
        .with_subtask(Subtask::new(60, "large").with_test(12, number(101..=1_000_000)))
        .with_partial_solution("small only", &wrong_when("n > 100 && n % 10 != 0"), &[0])
}

#[test]
fn results_report_time_and_memory() {
    let (dir, task) = test_task();
    two_subtasks(task).run_mode(Mode::Files).unwrap();

    let official = results_of(dir.path(), "Running official solution");
    assert_eq!(official.len(), 2, "{official:?}");
    for (line, tests) in official.iter().zip([6, 12]) {
        assert!(line.contains(&format!("OK ({tests}) ")) && line.contains("ms ") && line.ends_with("MB"), "{line}");
    }
}

#[test]
fn without_fail_fast_every_test_is_run() {
    let (dir, task) = test_task();
    two_subtasks(task).run_mode(Mode::Files).unwrap();

    let partial = results_of(dir.path(), "Running partial solution 1");
    assert!(partial[0].contains("OK (6)"), "{partial:?}");
    assert!(partial[1].contains("WA (") && !partial[1].contains("skipped"), "{partial:?}");
}

#[test]
fn fail_fast_skips_the_tests_after_the_first_failure() {
    let run = |jobs: usize| {
        let (dir, task) = test_task();
        two_subtasks(task).with_fail_fast(true).with_jobs(jobs).run_mode(Mode::Files).unwrap();
        let tests = test_files(dir.path());
        (results_of(dir.path(), "Running official solution"), results_of(dir.path(), "Running partial solution 1"), tests)
    };
    let (official, partial, tests) = run(4);

    assert!(official.iter().all(|line| !line.contains("skipped")), "the official solution is run on everything: {official:?}");
    assert!(partial[0].contains("OK (6)") && !partial[0].contains("skipped"), "a subtask meant to pass is run in full: {partial:?}");

    // Everything up to the first failed test is reported, and nothing after it.
    let second_subtask: Vec<i64> = tests
        .iter()
        .filter(|(name, _)| name.starts_with("test.02") && Path::new(name).extension().is_some_and(|extension| extension == "in"))
        .map(|(_, input)| input.trim().parse().unwrap())
        .collect();
    let passed_first = second_subtask.iter().take_while(|&&n| n % 10 == 0).count();
    let skipped = second_subtask.len() - passed_first - 1;
    assert!(skipped > 0, "the test is pointless unless something is skipped");
    let passed = if passed_first > 0 { format!("OK ({passed_first}) ") } else { String::new() };
    assert_eq!(partial[1], format!("- Subtask 2: {passed}WA (1) skipped ({skipped}) "));

    // What is reported does not depend on which runs happened to finish
    // first. The first subtask's line has measurements in it, which vary.
    for jobs in [1, 9] {
        let (_, other_partial, other_tests) = run(jobs);
        assert_eq!((&other_partial[1], &other_tests), (&partial[1], &tests), "with {jobs} jobs");
    }
}

#[test]
fn fail_fast_still_catches_a_partial_solution_that_passes_too_much() {
    let (_dir, task) = test_task();
    // The hunt is off, so only the final run can notice.
    let result = two_subtasks(task)
        .with_partial_solution("secretly right", DOUBLE, &[0])
        .with_min_failures(0)
        .with_fail_fast(true)
        .run_mode(Mode::Files);
    assert!(
        matches!(
            result,
            Err(Error::PartialSolutionPassesExtraSubtask {
                subtask_number: 2,
                partial_number: 2,
                ..
            })
        ),
        "got {result:?}"
    );
}

const ALLOCATE: &str = "#include <iostream>\n#include <vector>
    int main() { size_t megabytes; std::cin >> megabytes; std::vector<char> memory(megabytes << 20, 1); long long sum = 0; for (char byte : memory) sum += byte; std::cout << (sum > 0) << '\\n'; }";
const FRUGAL: &str = "#include <iostream>\nint main() { size_t megabytes; std::cin >> megabytes; std::cout << (megabytes > 0) << '\\n'; }";

fn megabytes(task: Task<String>, small: i64, large: i64) -> Task<String> {
    task.with_subtask(Subtask::new(50, "small").with_test(2, number(1..=small)))
        .with_subtask(Subtask::new(50, "large").with_test(2, number(large..=large + 20)))
        .with_memory_limit(64)
}

#[test]
fn a_partial_solution_over_the_memory_limit_fails() {
    let (dir, task) = test_task();
    megabytes(task, 8, 150)
        .with_solution_source(FRUGAL)
        .with_partial_solution("hungry", ALLOCATE, &[0])
        .with_min_failures(1)
        .run_mode(Mode::Files)
        .unwrap();

    let partial = results_of(dir.path(), "Running partial solution 1");
    assert!(partial[0].contains("OK (2)"), "{partial:?}");
    assert!(partial[1].contains("MLE (2)"), "{partial:?}");
}

#[test]
fn an_official_solution_over_the_memory_limit_is_an_error() {
    let (_dir, task) = test_task();
    let result = megabytes(task, 8, 150).with_solution_source(ALLOCATE).run_mode(Mode::Files);
    assert!(matches!(result, Err(Error::SolutionOutOfMemory { .. })), "got {result:?}");
}

#[test]
fn a_partial_solution_meant_to_pass_but_over_the_memory_limit_is_named() {
    let (_dir, task) = test_task();
    let result = megabytes(task, 8, 150)
        .with_solution_source(FRUGAL)
        .with_partial_solution("hungry", ALLOCATE, &[0, 1])
        .run_mode(Mode::Files);
    let Err(Error::PartialSolutionFailsSubtask { verdict, subtask_number, .. }) = &result else {
        unreachable!("got {result:?}")
    };
    assert_eq!((verdict.as_str(), *subtask_number), ("MLE", 2));
}

#[test]
fn without_a_memory_limit_nothing_is_over_it() {
    let (_dir, task) = test_task();
    let mut task = megabytes(task, 8, 150).with_solution_source(ALLOCATE);
    task.memory_limit_mb = None;
    task.run_mode(Mode::Files).unwrap();
}

#[test]
fn a_checker_may_be_a_closure_with_state() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let tolerance = 1;
    let (_dir, task) = test_task();
    task.with_solution_source(DOUBLE)
        .with_subtask(Subtask::new(40, "small").with_test(6, number(1..=100)))
        .with_subtask(Subtask::new(60, "large").with_test(12, number(101..=1_000_000)))
        .with_partial_solution("off by one", &wrong_when("true"), &[0, 1])
        .with_partial_solution("off by one or two", &wrong_when("true").replace("+ 1", "+ 1 + (n > 100)"), &[0])
        .with_checker(move |_input, official, output| {
            counted.fetch_add(1, Ordering::Relaxed);
            let parse = |text: &str| text.trim().parse::<i64>().unwrap_or(i64::MIN);
            (parse(official) - parse(output)).abs() <= tolerance
        })
        .run_mode(Mode::Files)
        .unwrap();
    assert!(calls.load(Ordering::Relaxed) > 18, "the checker was hardly called");
}

#[test]
fn fixed_tests_are_written_as_they_are_given() {
    for mode in [Mode::Files, Mode::Seeds] {
        let (dir, task) = test_task();
        let build = |task: Task<String>| {
            task.with_solution_source(DOUBLE)
                .with_subtask(Subtask::new(10, "samples").with_fixed_test("3\n".to_owned()).with_fixed_test("  14 ".to_owned()))
                .with_subtask(Subtask::new(90, "the rest").with_fixed_test("5\n".to_owned()).with_test(3, number(1..=1000)))
        };
        build(task).run_mode(mode).unwrap();

        let files = test_files(dir.path());
        assert_eq!(files.len(), 12);
        let mut served = Vec::new();
        for (_, contents) in &files[..4] {
            if mode == Mode::Files {
                served.push(contents.clone());
            } else {
                let mut answer = Vec::new();
                build(Task::new("served", dir.path())).serve_io(&mut contents.as_bytes(), &mut answer).unwrap();
                served.push(String::from_utf8(answer).unwrap());
            }
        }
        // In file name order: the inputs and outputs of the two samples, which are shuffled.
        served.sort();
        assert_eq!(served, ["14\n", "28\n", "3\n", "6\n"], "in {mode:?}");
    }
}

#[test]
#[should_panic(expected = "at least one solution")]
fn zero_jobs_are_rejected() {
    let _ = Task::<String>::new("jobs", Path::new("unused")).with_jobs(0);
}

#[test]
#[should_panic(expected = "n is too large")]
fn a_panicking_generator_ends_the_run_with_its_own_message() {
    let (_dir, task) = test_task();
    let _ = task
        .with_solution_source(DOUBLE)
        .with_subtask(Subtask::new(100, "all").with_test(8, number(1..=1000)).with_validator(|input: &String| {
            assert!(input.trim().parse::<i64>().unwrap() <= 0, "n is too large");
        }))
        .run_mode(Mode::Files);
}

#[test]
#[should_panic(expected = "the official solution was set twice")]
fn two_official_solutions_are_rejected() {
    let _ = Task::<String>::new("twice", Path::new("unused")).with_solution_source(DOUBLE).with_solution_source(DOUBLE);
}

/// Candidates are drawn several at a time, but one that drawing them one by one never reaches must
/// not be able to end the run.
#[test]
fn a_candidate_drawn_ahead_of_need_cannot_fail_the_run() {
    // The first candidate of the hunt: its generator is picked, then its seed drawn.
    let mut master = Rng::from_seed(DEFAULT_SEED);
    master.random_range(0..1_usize);
    let first = Rng::from_seed(master.next_u64()).random_range(1..=1_000_000_000_i64);

    for jobs in [1, 4, 16] {
        let (dir, task) = test_task();
        task.with_solution_source(DOUBLE)
            // Kept only to break the partial solution, which the very first candidate does.
            .with_subtask(Subtask::new(100, "all").with_test(0, move |rng| {
                let n = rng.random_range(1..=1_000_000_000_i64);
                assert_eq!(n, first, "a candidate after the first was built with {jobs} jobs and mattered");
                format!("{n}\n")
            }))
            .with_partial_solution("always wrong", &wrong_when("true"), &[])
            .with_min_failures(1)
            .with_jobs(jobs)
            .run_mode(Mode::Files)
            .unwrap();
        assert_eq!(test_files(dir.path()).len(), 2, "with {jobs} jobs");
    }
}
