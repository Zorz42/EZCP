use crate::rng::{Rng, SampleUniform};
use crate::to_output::Counted;
use std::ops::RangeBounds;

/// Arrays with a length from `len` and values from `value`, written as the length, then the values.
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

/// [`array_generator_custom`] with values from `values`.
pub fn array_generator<T, L, V>(len: L, values: V) -> impl Fn(&mut Rng) -> Counted<T> + Send + Sync
where
    T: SampleUniform,
    L: RangeBounds<usize> + Clone + Send + Sync,
    V: RangeBounds<T> + Clone + Send + Sync,
{
    array_generator_custom(len, move |rng| rng.random_range(values.clone()))
}
