use crate::tests::test_task;
use crate::{Error, Mode, Result, Subtask, Task};

fn printing(output: &str) -> String {
    format!("#include <iostream>\nint main() {{ std::cout << \"{output}\"; }}")
}

/// A checker reading the input, the output and the official output into `input`, `output` and
/// `official`, then exiting with `accepts ? 0 : 1`.
fn checker(accepts: &str) -> String {
    format!(
        "#include <fstream>\n#include <sstream>\n#include <string>
        std::string read(const char* path) {{ std::ifstream file(path); std::stringstream text; text << file.rdbuf(); return text.str(); }}
        int main(int argc, char** argv) {{ std::string input = read(argv[1]), output = read(argv[2]), official = read(argv[3]); return ({accepts}) ? 0 : 1; }}"
    )
}

/// Runs `main` and the partial solutions on one subtask of two empty tests.
fn run(checker: Option<String>, main: &str, partials: &[(&str, &[usize])]) -> Result<()> {
    let (_dir, task) = test_task();
    let mut task = task
        .with_solution_source(main)
        .with_subtask(Subtask::new(0, "only subtask").with_test(2, |_rng| String::new()))
        .with_min_failures(1)
        .with_max_tries(10);
    if let Some(checker) = checker {
        task = task.with_checker(&checker);
    }
    partials
        .iter()
        .fold(task, |task, (source, passes)| task.with_partial_solution("partial", source, passes))
        .run_mode(Mode::Files)
}

#[test]
fn default_checker_accepts_trailing_whitespace() {
    run(None, &printing("42\\n"), &[(&printing("42\\n\\n"), &[0])]).unwrap();
}

#[test]
fn default_checker_rejects_different_output() {
    run(None, &printing("1\\n"), &[(&printing("2\\n"), &[])]).unwrap();
}

#[test]
fn exact_checker_rejects_trailing_whitespace() {
    run(Some(checker("output == official")), &printing("42\\n"), &[(&printing("42   \\n"), &[])]).unwrap();
}

#[test]
fn always_accept_checker_accepts_any_output() {
    run(Some(checker("true")), &printing("42\\n"), &[(&printing("0\\n"), &[0])]).unwrap();
}

/// Nothing can break the bad solution, so the run has to fail.
#[test]
fn always_accept_checker_with_bad_solution() {
    let result = run(Some(checker("true")), &printing("1\\n"), &[(&printing("999\\n"), &[])]);
    assert!(
        matches!(
            result,
            Err(Error::PartialSolutionPassesExtraSubtask {
                subtask_number: 1,
                partial_number: 1,
                ..
            })
        ),
        "got {result:?}"
    );
}

#[test]
fn always_reject_checker_errors_on_main_solution() {
    let result = run(Some(checker("false")), &printing("1\\n"), &[]);
    assert!(matches!(result, Err(Error::SolutionFailed { .. })), "got {result:?}");
}

#[test]
fn checker_rejects_good_partial_solution_errors() {
    let result = run(Some(checker("output.rfind(\"main\", 0) == 0")), &printing("main\\n"), &[(&printing("partial\\n"), &[0])]);
    assert!(matches!(result, Err(Error::PartialSolutionFailsSubtask { .. })), "got {result:?}");
}

#[test]
fn any_nonneg_integer_checker_end_to_end() {
    let echo = |answer: &str| format!("#include <iostream>\nint main() {{ int n; std::cin >> n; std::cout << {answer} << '\\n'; }}");
    let (_dir, task): (_, Task<String>) = test_task();
    task.with_checker(&checker("!output.empty() && output.find_first_not_of(\"0123456789\\n\") == std::string::npos"))
        .with_solution_source(&echo("n"))
        .with_subtask(Subtask::new(0, "only subtask").with_test(0, |rng| rng.random_range(1_i32..=100).to_string()))
        .with_partial_solution("good", &echo("0"), &[0])
        .with_partial_solution("bad", &echo("-1"), &[])
        .with_min_failures(3)
        .run_mode(Mode::Files)
        .unwrap();
}

#[test]
fn a_checker_reads_the_input() {
    let (_dir, task): (_, Task<String>) = test_task();
    let echo = "#include <iostream>\nint main() { int n; std::cin >> n; std::cout << n + 1 << '\\n'; }";
    task.with_checker(&checker("std::stoi(output) > std::stoi(input)"))
        .with_solution_source(echo)
        .with_subtask(Subtask::new(0, "only subtask").with_test(5, |rng| rng.random_range(1_i32..=100).to_string()))
        .with_partial_solution("good", &echo.replace("n + 1", "n + 50"), &[0])
        .with_partial_solution("bad", &echo.replace("n + 1", "n"), &[])
        .with_min_failures(3)
        .run_mode(Mode::Files)
        .unwrap();
}

#[test]
fn a_failing_checker_is_an_error() {
    let result = run(Some("int main() { return 7; }".to_owned()), &printing("1\\n"), &[]);
    assert!(matches!(result, Err(Error::CheckerFailed { .. })), "got {result:?}");
}

#[test]
fn a_checker_that_does_not_compile_is_an_error() {
    let result = run(Some("int main() { nope }".to_owned()), &printing("1\\n"), &[]);
    assert!(matches!(result, Err(Error::CompilerError { .. })), "got {result:?}");
}
