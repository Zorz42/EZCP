/// Each run of whitespace becomes one newline if it holds one and one space otherwise; no leading
/// whitespace, and a single newline at the end.
pub fn trim_whitespace(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for line in input.lines() {
        let mut words = line.split_whitespace();
        if let Some(first) = words.next() {
            result.push_str(first);
            for word in words {
                result.push(' ');
                result.push_str(word);
            }
            result.push('\n');
        }
    }
    if result.is_empty() {
        result.push('\n');
    }
    result
}

/// Whether [`trim_whitespace`] would leave `text` as it is: the usual case, and far cheaper to find out
/// than to rebuild. May say `false` wrongly, never `true`.
fn is_trimmed(text: &str) -> bool {
    if text == "\n" {
        return true;
    }
    // So that a leading space or newline is caught.
    let mut previous = b'\n';
    for &byte in text.as_bytes() {
        let allowed = match byte {
            b' ' | b'\n' => previous != b' ' && previous != b'\n',
            // Other ASCII whitespace, and whatever might be Unicode whitespace.
            b'\t' | 0x0B | 0x0C | b'\r' | 0x80.. => false,
            _ => true,
        };
        if !allowed {
            return false;
        }
        previous = byte;
    }
    !text.is_empty() && previous == b'\n'
}

/// Does not copy text that is already trimmed.
pub fn trim_whitespace_owned(text: String) -> String {
    if is_trimmed(&text) { text } else { trim_whitespace(&text) }
}
