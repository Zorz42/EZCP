use indicatif::{MultiProgress, ProgressBar};
use std::ops::Deref;

/// A progress bar that takes itself off the screen once it goes out of scope.
///
/// Almost everything that shows a bar can also give up through `?` part of the
/// way, and a bar that is only removed on the happy path stays behind as a frozen
/// leftover.
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
