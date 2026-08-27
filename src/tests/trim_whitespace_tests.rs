#[cfg(test)]
mod trim_whitespace_tests {
    use crate::create_tests::trim_whitespace;

    #[test]
    fn collapses_runs_of_spaces() {
        assert_eq!(trim_whitespace("1   2     3"), "1 2 3\n");
    }

    #[test]
    fn a_run_containing_a_newline_becomes_a_newline() {
        assert_eq!(trim_whitespace("1 2 \n  3"), "1 2\n3\n");
        assert_eq!(trim_whitespace("1\n\n\n2"), "1\n2\n");
    }

    #[test]
    fn drops_leading_whitespace() {
        assert_eq!(trim_whitespace("   1 2"), "1 2\n");
        assert_eq!(trim_whitespace("\n\n1 2"), "1 2\n");
        assert_eq!(trim_whitespace("\t 1"), "1\n");
    }

    #[test]
    fn drops_trailing_whitespace_and_ensures_one_newline() {
        assert_eq!(trim_whitespace("1 2   "), "1 2\n");
        assert_eq!(trim_whitespace("1 2\n\n\n"), "1 2\n");
        assert_eq!(trim_whitespace("1 2\n"), "1 2\n");
    }

    #[test]
    fn normalises_windows_line_endings() {
        assert_eq!(trim_whitespace("1 2\r\n3 4\r\n"), "1 2\n3 4\n");
    }

    #[test]
    fn whitespace_only_input_becomes_a_single_newline() {
        assert_eq!(trim_whitespace(""), "\n");
        assert_eq!(trim_whitespace("   "), "\n");
        assert_eq!(trim_whitespace("\n\n"), "\n");
    }

    #[test]
    fn is_idempotent() {
        for input in ["  1  2 \n\n 3 ", "1\n2\n", "", "   ", "a\r\nb"] {
            let once = trim_whitespace(input);
            assert_eq!(trim_whitespace(&once), once, "not idempotent for {input:?}");
        }
    }
}
