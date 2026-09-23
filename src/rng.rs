//! xoshiro256++ seeded through `SplitMix64`, with Lemire's bounded sampling.
//!
//! Implemented here rather than taken from a crate because published seeds must
//! rebuild the same tests forever, and crates only keep values stable within a
//! major version. Nothing here may change the stream of numbers.

use std::hash::{BuildHasher as _, Hasher as _, RandomState};
use std::ops::{Bound, RangeBounds};

/// A seeded random number generator that produces the same sequence on every
/// platform and in every version of EZCP.
///
/// ```
/// # use ezcp::Rng;
/// let mut rng = Rng::from_seed(42);
/// let a = rng.random_range(0..100);
/// let b = Rng::from_seed(42).random_range(0..100);
/// assert_eq!(a, b);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: [u64; 4],
}

/// Expands a seed into the state, so that consecutive seeds give unrelated
/// streams.
const fn split_mix_64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    /// Creates a generator from a seed. Every seed is valid, including zero.
    #[must_use]
    pub const fn from_seed(seed: u64) -> Self {
        let mut mixer = seed;
        let state = [split_mix_64(&mut mixer), split_mix_64(&mut mixer), split_mix_64(&mut mixer), split_mix_64(&mut mixer)];
        Self { state }
    }

    /// Creates a generator from an unpredictable seed.
    #[must_use]
    pub fn from_entropy() -> Self {
        Self::from_seed(random_seed())
    }

    /// Returns the next value of the stream.
    pub const fn next_u64(&mut self) -> u64 {
        let result = self.state[0].wrapping_add(self.state[3]).rotate_left(23).wrapping_add(self.state[0]);

        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);

        result
    }

    /// Returns a seed for a separate generator, such as the one a single test
    /// is generated with.
    pub const fn next_seed(&mut self) -> u64 {
        self.next_u64()
    }

    /// Returns a uniform value below `bound`, or any `u64` if `bound` is zero.
    const fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return self.next_u64();
        }

        let mut product = (self.next_u64() as u128).wrapping_mul(bound as u128);
        let mut low = product as u64;
        if low < bound {
            // Draws below this would make some values more likely than others.
            let threshold = bound.wrapping_neg() % bound;
            while low < threshold {
                product = (self.next_u64() as u128).wrapping_mul(bound as u128);
                low = product as u64;
            }
        }
        (product >> 64) as u64
    }

    /// Returns a uniform value from an integer range of any kind.
    ///
    /// # Panics
    /// Panics if the range is empty.
    pub fn random_range<T: SampleUniform, R: RangeBounds<T>>(&mut self, range: R) -> T {
        T::sample(self, range)
    }

    /// Returns `true` with probability `probability`.
    ///
    /// # Panics
    /// Panics if `probability` is not between 0 and 1.
    pub fn random_bool(&mut self, probability: f64) -> bool {
        // The full f64 mantissa, so every probability is represented exactly.
        const SCALE: f64 = (1_u64 << 53) as f64;

        assert!((0.0..=1.0).contains(&probability), "a probability has to be between 0 and 1, got {probability}");
        let threshold = (probability * SCALE) as u64;
        (self.next_u64() >> 11) < threshold
    }

    /// Returns a uniform value in `[0, 1)`.
    pub const fn random_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// Shuffles a slice uniformly (Fisher-Yates).
    pub const fn shuffle<T>(&mut self, slice: &mut [T]) {
        let mut i = slice.len();
        while i > 1 {
            i -= 1;
            let j = self.below(i as u64 + 1) as usize;
            slice.swap(i, j);
        }
    }

    /// Returns a random element of `slice`, or `None` if it is empty.
    pub const fn choose<'slice, T>(&mut self, slice: &'slice [T]) -> Option<&'slice T> {
        if slice.is_empty() {
            return None;
        }
        let idx = self.below(slice.len() as u64) as usize;
        Some(&slice[idx])
    }
}

/// `RandomState` is the only OS randomness in the standard library.
fn random_seed() -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since_epoch| since_epoch.as_nanos() as u64),
    );
    hasher.write_usize(std::process::id() as usize);
    hasher.finish()
}

/// An integer type that [`Rng::random_range`] can produce.
pub trait SampleUniform: Sized {
    /// Draws one value from `range`.
    fn sample<R: RangeBounds<Self>>(rng: &mut Rng, range: R) -> Self;
}

/// Samples an offset from `low` in `u64`, where the distance between any two
/// values of these types fits.
macro_rules! impl_sample_uniform {
    ($($int:ty),*) => {
        $(
            impl SampleUniform for $int {
                fn sample<R: RangeBounds<Self>>(rng: &mut Rng, range: R) -> Self {
                    let low = match range.start_bound() {
                        Bound::Included(&low) => low,
                        // No integer range syntax produces an excluded start.
                        Bound::Excluded(&low) => low.checked_add(1).unwrap_or_else(|| empty_range()),
                        Bound::Unbounded => Self::MIN,
                    };
                    let high = match range.end_bound() {
                        Bound::Included(&high) => high,
                        Bound::Excluded(&high) => high.checked_sub(1).unwrap_or_else(|| empty_range()),
                        Bound::Unbounded => Self::MAX,
                    };

                    if low > high {
                        empty_range();
                    }

                    // Wrapping, so it is the distance even when the range spans zero.
                    let span = (high as u64).wrapping_sub(low as u64);
                    // Wraps to zero, meaning "any value", for a range covering all of `u64`.
                    let offset = rng.below(span.wrapping_add(1));
                    (low as u64).wrapping_add(offset) as Self
                }
            }
        )*
    };
}

impl_sample_uniform!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

#[cold]
#[allow(clippy::panic)]
fn empty_range() -> ! {
    panic!("cannot draw a random value from an empty range");
}
