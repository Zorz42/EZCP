use indicatif::{MultiProgress, ProgressBar};
use std::ops::Deref;

/// A progress bar that is removed when dropped, including on an early `?` return.
pub struct ScopedProgressBar<'logger> {
    logger: &'logger MultiProgress,
    bar: ProgressBar,
}

impl<'logger> ScopedProgressBar<'logger> {
    pub fn new(logger: &'logger MultiProgress, len: u64) -> Self {
        Self {
            logger,
            bar: logger.add(ProgressBar::new(len)),
        }
    }
}

impl Deref for ScopedProgressBar<'_> {
    type Target = ProgressBar;

    fn deref(&self) -> &Self::Target {
        &self.bar
    }
}

impl Drop for ScopedProgressBar<'_> {
    fn drop(&mut self) {
        self.logger.remove(&self.bar);
    }
}
