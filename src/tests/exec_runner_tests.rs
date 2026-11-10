#[cfg(test)]
mod exec_runner_tests {
    use crate::runner::exec_runner::{TimerReport, parse_result_marker};

    /// What the timer reported, as `(verdict, time_ms, memory_kb)`.
    fn parse(stderr: &str) -> Option<(&str, i32, u64)> {
        parse_result_marker(stderr).map(|TimerReport { verdict, time_ms, memory_kb }| (verdict, time_ms, memory_kb))
    }

    #[test]
    fn parses_verdict_time_and_memory() {
        assert_eq!(parse("\n__EZCP_RESULT__ OK 42 1536\n"), Some(("OK", 42, 1536)));
    }

    #[test]
    fn ignores_output_the_solution_wrote_to_stderr() {
        let stderr = "debug line\nno trailing newline__EZCP_RESULT__ TLE 1234\n";
        assert_eq!(parse(stderr), Some(("TLE", 1234, 0)));
    }

    #[test]
    fn last_marker_wins_so_a_solution_cannot_spoof_the_verdict() {
        let stderr = "__EZCP_RESULT__ OK 0 1\nreal:\n__EZCP_RESULT__ RTE 7 9\n";
        assert_eq!(parse(stderr), Some(("RTE", 7, 9)));
    }

    #[test]
    fn tolerates_windows_line_endings() {
        assert_eq!(parse("\r\n__EZCP_RESULT__ OK 5 3\r\n"), Some(("OK", 5, 3)));
    }

    #[test]
    fn missing_marker_is_reported_as_missing() {
        assert_eq!(parse("segmentation fault\n"), None);
    }

    #[test]
    fn missing_or_bogus_numbers_do_not_panic() {
        assert_eq!(parse("__EZCP_RESULT__ OK\n"), Some(("OK", 0, 0)));
        assert_eq!(parse("__EZCP_RESULT__ OK 7\n"), Some(("OK", 7, 0)));
        assert_eq!(parse("__EZCP_RESULT__ OK nonsense -5\n"), Some(("OK", 0, 0)));
        assert_eq!(parse("__EZCP_RESULT__ OK 99999999999999 99999999999999\n"), Some(("OK", i32::MAX, 99_999_999_999_999)));
        assert_eq!(parse("__EZCP_RESULT__"), None);
    }
}
