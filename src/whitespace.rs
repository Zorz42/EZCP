//! Whitespace normalisation of inputs and outputs.

/// Turns each run of whitespace into one newline if it contains one and one
/// space otherwise, drops leading whitespace and ends with a single newline.
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

/// Whether [`trim_whitespace`] would return `text` unchanged, which is the usual
/// case and far cheaper to find out than to rebuild the text.
///
/// May say `false` for text that is in fact unchanged, but never `true` for
/// text that is not.
fn is_trimmed(text: &str) -> bool {
    // An empty line is only right as the whole text.
    if text == "\n" {
        return true;
    }
    // At the start, a space or a newline would be leading whitespace.
    let mut previous = b'\n';
    for &byte in text.as_bytes() {
        let allowed = match byte {
            // Neither leading, nor doubled, nor an empty line.
            b' ' | b'\n' => previous != b' ' && previous != b'\n',
            // The other ASCII whitespace, and anything that might be Unicode whitespace.
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

/// [`trim_whitespace`], without copying text that is already trimmed.
pub fn trim_whitespace_owned(text: String) -> String {
    if is_trimmed(&text) { text } else { trim_whitespace(&text) }
}
