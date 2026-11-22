use crate::report::{Mismatch, Verdict};
use crate::rng::Rng;
use crate::tests::{DOUBLE, test_files, test_task, wrong_when};
use crate::{Compiler, Error, Mode, Subtask, Task};
use log::LevelFilter;
use serde_json::{Value, json};

fn number(range: std::ops::RangeInclusive<i64>) -> impl Fn(&mut Rng) -> String + Send + Sync {
    move |rng| format!("{}\n", rng.random_range(range.clone()))
}

fn two_subtasks(task: Task<String>) -> Task<String> {
    task.with_solution_source(DOUBLE)
        .with_subtask(Subtask::new(50, "small").with_test(6, number(1..=100)))
        .with_subtask(Subtask::new(50, "large").with_test(6, number(101..=1_000_000)))
}

#[test]
fn keep_going_records_every_mismatch_and_writes_no_tests() {
    let (dir, task) = test_task();
    let report = two_subtasks(task)
        // Declared to pass the large subtask, which it fails.
        .with_partial_solution("small only", &wrong_when("n > 100"), &[0, 1])
        // Declared to fail the small subtask, which it passes.
        .with_partial_solution("correct", DOUBLE, &[1])
        .with_max_tries(10)
        .with_keep_going(true)
        .run_report(Mode::Files);

    assert!(matches!(report.error, Some(Error::DeclarationsMismatch { count: 2 })), "got {:?}", report.error);
    // In the order the subtasks are generated.
    assert!(matches!(report.mismatches.first(), Some(Mismatch::Passes { solution: 1, subtask: 0 })), "got {:?}", report.mismatches);
    let Some(Mismatch::Fails { solution: 0, subtask: 1, verdict: Verdict::WrongAnswer, test }) = report.mismatches.get(1) else {
        unreachable!("got {:?}", report.mismatches);
    };
    assert!(test.input.trim().parse::<i64>().unwrap() > 100);
    assert_eq!(test.output.as_deref().map(str::trim), test.input.trim().parse::<i64>().ok().map(|n| (2 * n + 1).to_string()).as_deref());
    // Both still ran on every finished test.
    assert!(report.partials[0].results[0].passed() && !report.partials[0].results[1].passed());
    assert!(report.partials[1].results[0].passed() && report.partials[1].results[1].passed());
    assert_eq!(test_files(dir.path()), []);
}

#[test]
fn the_json_report_has_results_mismatches_and_tests() {
    let (_dir, task) = test_task();
    let report = two_subtasks(task).with_partial_solution("small only", &wrong_when("n > 100"), &[0]).run_report(Mode::Files);
    assert!(report.error.is_none(), "got {:?}", report.error);

    let json: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(json["passed"], true);
    assert_eq!(json["error"], Value::Null);
    assert_eq!(json["subtasks"][1], json!({ "name": "large", "points": 50 }));
    assert_eq!(json["official"][0]["verdicts"]["OK"], 6);
    assert_eq!(json["partials"][0]["declared"], json!([0]));
    assert_eq!(json["partials"][0]["results"][0]["passed"], true);
    assert_eq!(json["partials"][0]["results"][1]["passed"], false);
    assert!(json["partials"][0]["results"][1]["first_failure"].is_u64());
    assert_eq!(json["mismatches"], json!([]));
    assert_eq!(json["tests"].as_array().map(Vec::len), Some(report.tests.len()));
    assert_eq!(json["tests"][0]["input_file"], "test.01.001.in");
}

#[test]
fn an_error_is_in_the_json_report_by_kind() {
    let (_dir, task) = test_task::<String>();
    let json: Value = serde_json::from_str(&task.run_report(Mode::Files).to_json()).unwrap();
    assert_eq!(json["passed"], false);
    assert_eq!(json["error"]["kind"], "MissingSolution");
}

#[test]
fn the_outcome_cache_spares_runs_it_has_seen() {
    let (dir, task) = test_task();
    let log = dir.path().join("runs.log");
    let counting = format!(
        "#include <cstdio>\nint main() {{ long long n; if (scanf(\"%lld\", &n) != 1) return 1; FILE* f = fopen({:?}, \"a\"); fputs(\"x\\n\", f); fclose(f); printf(\"%lld\\n\", 2 * n); }}",
        log.display().to_string()
    );
    let build = |task: Task<String>| {
        task.with_solution_source(DOUBLE)
            .with_subtask(Subtask::new(100, "all").with_test(5, number(1..=1_000_000_000)))
            .with_partial_solution("counting", &counting, &[0])
            .with_outcome_cache(true)
    };
    let runs = || std::fs::read_to_string(&log).unwrap().lines().count();

    build(task).run_mode(Mode::Files).unwrap();
    // The final run finds what generation ran.
    assert_eq!(runs(), 5);
    build(Task::new("again", dir.path()).with_debug_level(LevelFilter::Trace)).run_mode(Mode::Files).unwrap();
    assert_eq!(runs(), 5);
}

#[test]
fn a_partial_min_failures_override_ends_the_hunt_sooner() {
    for (min_failures, files) in [(None, 10), (Some(1), 2)] {
        let (dir, task) = test_task();
        let task = task
            .with_solution_source(DOUBLE)
            .with_subtask(Subtask::new(100, "all").with_test(0, number(1..=1_000_000)))
            .with_partial_solution("always wrong", &wrong_when("true"), &[]);
        let task = match min_failures {
            Some(n) => task.with_partial_min_failures(0, n),
            None => task,
        };
        task.run_mode(Mode::Files).unwrap();
        assert_eq!(test_files(dir.path()).len(), files, "with {min_failures:?}");
    }
}

#[test]
fn the_compiler_fills_the_build_folder_a_task_uses() {
    let (dir, task) = test_task();
    let build = dir.path().join("build");
    let compiler = Compiler::new(&build).unwrap();
    compiler.compile(DOUBLE).unwrap();
    let Err(Error::CompilerError { stderr, .. }) = compiler.compile("int main() { nope }") else {
        unreachable!("a broken source compiled");
    };
    assert!(stderr.contains("nope"), "{stderr}");

    let executable = std::fs::read_dir(&build)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_none_or(|extension| extension == "exe"))
        .unwrap();
    let modified = || std::fs::metadata(&executable).unwrap().modified().unwrap();
    let before = modified();
    task.with_solution_source(DOUBLE).with_subtask(Subtask::new(100, "all").with_test(2, number(1..=100))).run_mode(Mode::Files).unwrap();
    assert_eq!(modified(), before);
}

#[test]
fn a_retry_keeps_the_best_run() {
    use crate::outcome::{Outcome, better};
    let run = |verdict, time_ms| Outcome { verdict, time_ms, memory_kb: 0 };
    assert_eq!(better(run(Verdict::TimedOut, 0), run(Verdict::Ok, 900)), run(Verdict::Ok, 900));
    assert_eq!(better(run(Verdict::Ok, 900), run(Verdict::Ok, 700)), run(Verdict::Ok, 700));
    assert_eq!(better(run(Verdict::Ok, 700), run(Verdict::TimedOut, 0)), run(Verdict::Ok, 700));
}
