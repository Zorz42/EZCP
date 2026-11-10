use crate::rng::{Rng, SampleUniform};
use crate::to_output::Counted;
use std::fmt::Display;
use std::ops::RangeBounds;

/// Formats `[1, 2, 3]` as `"3\n1 2 3\n"`, or as `"1 2 3\n"` without the count.
#[must_use]
pub fn array_to_string<T: Display>(array: &[T], include_count: bool) -> String {
    let values = array.iter().map(T::to_string).collect::<Vec<_>>().join(" ");
    if include_count { format!("{}\n{values}\n", array.len()) } else { format!("{values}\n") }
}

/// Returns a generator of arrays whose length is drawn from `len` and whose
/// values are drawn by `value`. An array is written as its length on one line
/// and its values on the next.
///
/// ```
/// use ezcp::{Subtask, array_generator_custom};
///
/// // Even numbers only.
/// let subtask = Subtask::new(100, "n <= 1000").with_test(5, array_generator_custom(1..=1000, |rng| rng.random_range(0..=500) * 2));
/// ```
pub fn array_generator_custom<T, L, F>(len: L, value: F) -> impl Fn(&mut Rng) -> Counted<T> + Send + Sync
where
    L: RangeBounds<usize> + Clone + Send + Sync,
    F: Fn(&mut Rng) -> T + Send + Sync,
{
    move |rng| {
        let len = rng.random_range(len.clone());
        Counted((0..len).map(|_| value(rng)).collect())
    }
}

/// Returns a generator of arrays whose length is drawn from `len` and whose
/// values are drawn from `values`. An array is written as its length on one
/// line and its values on the next.
///
/// ```
/// use ezcp::{Subtask, array_generator};
///
/// let subtask = Subtask::new(100, "n <= 1000").with_test(5, array_generator(1..=1000, 1..=1_000_000_000_i64));
/// ```
pub fn array_generator<T, L, V>(len: L, values: V) -> impl Fn(&mut Rng) -> Counted<T> + Send + Sync
where
    T: SampleUniform,
    L: RangeBounds<usize> + Clone + Send + Sync,
    V: RangeBounds<T> + Clone + Send + Sync,
{
    array_generator_custom(len, move |rng| rng.random_range(values.clone()))
}
