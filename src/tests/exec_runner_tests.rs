#[cfg(test)]
mod exec_runner_tests {
    use crate::runner::exec_runner::parse_result_marker;

    #[test]
    fn parses_verdict_and_time() {
        assert_eq!(parse_result_marker("\n__EZCP_RESULT__ OK 42\n"), Some(("OK", 42)));
    }

    #[test]
    fn ignores_output_the_solution_wrote_to_stderr() {
        let stderr = "debug line\nno trailing newline__EZCP_RESULT__ TLE 1234\n";
        assert_eq!(parse_result_marker(stderr), Some(("TLE", 1234)));
    }

    #[test]
    fn last_marker_wins_so_a_solution_cannot_spoof_the_verdict() {
        let stderr = "__EZCP_RESULT__ OK 0\nreal:\n__EZCP_RESULT__ RTE 7\n";
        assert_eq!(parse_result_marker(stderr), Some(("RTE", 7)));
    }

    #[test]
    fn tolerates_windows_line_endings() {
        assert_eq!(parse_result_marker("\r\n__EZCP_RESULT__ OK 5\r\n"), Some(("OK", 5)));
    }

    #[test]
    fn missing_marker_is_reported_as_missing() {
        assert_eq!(parse_result_marker("segmentation fault\n"), None);
    }

    #[test]
    fn missing_or_bogus_time_does_not_panic() {
        assert_eq!(parse_result_marker("__EZCP_RESULT__ OK\n"), Some(("OK", 0)));
        assert_eq!(parse_result_marker("__EZCP_RESULT__ OK nonsense\n"), Some(("OK", 0)));
        assert_eq!(parse_result_marker("__EZCP_RESULT__ OK 99999999999999\n"), Some(("OK", i32::MAX)));
        assert_eq!(parse_result_marker("__EZCP_RESULT__"), None);
    }
}
