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

    // split the message into lines
    let message = record.args().to_string();
    for (line_num, line) in message.lines().enumerate() {
        if line_num == 0 {
            // For the first line, write the prefix
            writeln!(buf, "[{prefix}] {line}")?;
        } else {
            // For subsequent lines, just write the line
            let prefix_space = " ".repeat(prefix.len());
            writeln!(buf, "{prefix_space}|  {line}")?;
        }
    }
    Ok(())
}