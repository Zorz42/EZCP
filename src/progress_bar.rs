use crate::logger::Logger;
use std::io::Write;

pub const ANSI_RESET: &str = "\x1b[0m";
pub const ANSI_BOLD: &str = "\x1b[1m";
pub const ANSI_BLUE: &str = "\x1b[36m";
pub const ANSI_YELLOW: &str = "\x1b[33m";
pub const ANSI_RED: &str = "\x1b[91m";
pub const ANSI_GREEN: &str = "\x1b[92m";
pub const ANSI_GREY: &str = "\x1b[90m";

/// Prints a progress bar to stdout.
pub fn print_progress_bar(progress: f32, logger: &Logger) {
    let size = termsize::get();
    logger.log(format!("\r {ANSI_BLUE}{ANSI_BOLD}{:.2}%{ANSI_RESET} [", progress * 100.0));

    let bar_length = size.map_or(10, |size| (size.cols as usize - 15).max(0));
    let num_filled = (progress * bar_length as f32) as usize;
    let num_empty = ((bar_length - num_filled) as i32 - 1).max(0);

    logger.log(ANSI_GREY);
    logger.log(ANSI_BOLD);
    for _ in 0..num_filled {
        logger.log("=");
    }
    if num_filled > 0 {
        logger.log(">");
    }
    for _ in 0..num_empty {
        logger.log(" ");
    }
    logger.log(ANSI_RESET);
    logger.log("]");

    std::io::stdout().flush().ok();
}

/// Clears the progress bar from stdout.
pub fn clear_progress_bar(logger: &Logger) {
    let size = termsize::get();
    let bar_length = size.map_or(10, |size| size.cols as usize);

    logger.log("\r");
    for _ in 0..bar_length {
        logger.log(" ");
    }
    logger.log("\r");
    std::io::stdout().flush().ok();
}
