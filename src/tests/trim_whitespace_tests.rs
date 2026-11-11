use crate::rng::Rng;
use crate::whitespace::{trim_whitespace, trim_whitespace_owned};

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

// Already trimmed text is handed back rather than rebuilt, which must never change what comes out.

#[test]
fn agrees_with_trim_whitespace() {
    let pieces = ["a", "12", " ", " ", "\n", "\n", "\t", "\r\n", "\r", "\u{b}", "\u{c}", "\u{a0}", "\u{2028}", "\u{e9}", "-"];
    let mut rng = Rng::from_seed(99);
    for _ in 0..50_000 {
        let text: String = (0..rng.random_range(0..9)).map(|_| *rng.choose(&pieces).unwrap_or(&"")).collect();
        assert_eq!(trim_whitespace_owned(text.clone()), trim_whitespace(&text), "for {text:?}");
        // Trimming the ends first, as outputs once were, changes nothing either.
        assert_eq!(trim_whitespace(&(text.trim().to_owned() + "\n")), trim_whitespace(&text), "for {text:?}");
    }
}

#[test]
fn trimmed_text_is_not_copied() {
    for text in ["\n", "1\n", "1 2 3\n", "3\n1 2 3\n4 5\n", "-7 abc\n"] {
        let owned = text.to_owned();
        let address = owned.as_ptr();
        let trimmed = trim_whitespace_owned(owned);
        assert_eq!(trimmed, text);
        assert_eq!(trimmed.as_ptr(), address, "{text:?} was rebuilt");
    }
}

#[test]
fn anything_else_is_trimmed() {
    for (text, expected) in [
        ("", "\n"),
        ("1", "1\n"),
        (" 1\n", "1\n"),
        ("1 \n", "1\n"),
        ("1  2\n", "1 2\n"),
        ("1\n\n2\n", "1\n2\n"),
        ("\n1\n", "1\n"),
        ("1\n\n", "1\n"),
        ("1\t2\n", "1 2\n"),
        ("1\r\n", "1\n"),
        ("1\u{a0}2\n", "1 2\n"),
        ("\u{e9}\n", "\u{e9}\n"),
    ] {
        assert_eq!(trim_whitespace_owned(text.to_owned()), expected, "for {text:?}");
    }
}
