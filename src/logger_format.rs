use env_logger::fmt::Formatter;
use log::Record;
use std::io::Write;

pub fn logger_format(buf: &mut Formatter, record: &Record) -> std::io::Result<()> {
    let prefix = match record.level() {
        log::Level::Error => "ERROR",
        log::Level::Warn => "WARN",
        log::Level::Info => "*",
        log::Level::Debug => "D",
        log::Level::Trace => "T",
    };

    // Continuation lines line up under the first one instead of repeating the
    // prefix, so a multi-line message still reads as a single entry.
    let continuation_indent = " ".repeat(prefix.len());

    let message = record.args().to_string();
    for (line_num, line) in message.lines().enumerate() {
        if line_num == 0 {
            writeln!(buf, "[{prefix}] {line}")?;
        } else {
            writeln!(buf, "{continuation_indent}|  {line}")?;
        }
    }
    Ok(())
}
