use std::io::Write;
use crate::logger::Logger;

/// Prints a progress bar to stdout.
pub fn print_progress_bar(progress: f32, logger: &Logger) {
    let size = termsize::get();
    logger.log(format!("\r {:.2}% [", progress * 100.0));

    let bar_length = size.map_or(10, |size| (size.cols as usize - 10).max(0));
    let num_filled = (progress * bar_length as f32) as usize;
    let num_empty = bar_length - num_filled;

    for _ in 0..num_filled {
        logger.log("=");
    }
    if num_filled > 0 {
        logger.log(">");
    }
    for _ in 0..num_empty {
        logger.log(" ");
    }
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