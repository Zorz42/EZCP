//! A deterministic random number generator.
//!
//! Test generation has to be reproducible: an online judge stores a seed and
//! expects the same test bytes back from it months later, on another machine,
//! after another `cargo update`. That rules out `rand`'s thread RNG, which is
//! seeded from the operating system, and it also rules out leaning on any
//! external crate's sampling algorithms, because those are only value-stable
//! within a major version.
//!
//! So the algorithm lives here. It is xoshiro256++ seeded through `SplitMix64`,
//! with Lemire's unbiased bounded sampling on top. None of it is allowed to
//! change: a different stream of numbers is a different set of tests for every
//! task that has already published its seeds.

use std::hash::{BuildHasher as _, Hasher as _, RandomState};
use std::ops::{Bound, RangeBounds};

/// A seeded, reproducible random number generator.
///
/// The same seed always produces the same sequence, on every platform and in
/// every version of EZCP. Generators receive one of these and must not use any
/// other source of randomness, or the tests they produce cannot be regenerated
/// from their seed.
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

/// Mixes one 64-bit value into another, as `SplitMix64` does.
///
/// Used to turn a seed into a full state: a caller that passes 0, 1, 2, ... must
/// still get unrelated streams, which a raw copy of the seed into the state
/// would not give.
const fn split_mix_64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    /// Creates a generator from a seed.
    ///
    /// Every seed is valid, including zero.
    #[must_use]
    pub const fn from_seed(seed: u64) -> Self {
        let mut mixer = seed;
        let state = [split_mix_64(&mut mixer), split_mix_64(&mut mixer), split_mix_64(&mut mixer), split_mix_64(&mut mixer)];
        Self { state }
    }

    /// Creates a generator from an unpredictable seed.
    ///
    /// This is the one place in EZCP where randomness is not reproducible, and it
    /// exists only for `--seed random`. The seed it picked is always reported and
    /// written to `results.txt`, so the run can be repeated afterwards.
    #[must_use]
    pub fn from_entropy() -> Self {
        Self::from_seed(random_seed())
    }

    /// Returns the next value of the underlying xoshiro256++ stream.
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

    /// Returns a seed for a generator that has to run independently of this one.
    ///
    /// Drawing a seed rather than sharing the generator is what keeps a single
    /// test reproducible on its own: it does not matter how many candidates were
    /// drawn before it.
    pub const fn next_seed(&mut self) -> u64 {
        self.next_u64()
    }

    /// Returns a uniformly distributed value below `bound`.
    ///
    /// `bound` of zero means "no bound" and returns any `u64`, which is what the
    /// full-width integer ranges need.
    ///
    /// Lemire's method: multiply a random 64-bit word by the bound and keep the
    /// high half, rejecting the one short interval that would otherwise be
    /// sampled once too often. Rejection is what makes it unbiased, and it almost
    /// never happens.
    const fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return self.next_u64();
        }

        let mut product = (self.next_u64() as u128).wrapping_mul(bound as u128);
        let mut low = product as u64;
        if low < bound {
            // Everything below this threshold belongs to a bucket that got one
            // more value than the others, so those draws are thrown away.
            let threshold = bound.wrapping_neg() % bound;
            while low < threshold {
                product = (self.next_u64() as u128).wrapping_mul(bound as u128);
                low = product as u64;
            }
        }
        (product >> 64) as u64
    }

    /// Returns a uniformly distributed value from `range`.
    ///
    /// Every kind of integer range works: `0..n`, `1..=n`, `..=n` and `..` all
    /// mean what they do elsewhere in Rust.
    ///
    /// # Panics
    /// Panics if the range is empty, in the same way indexing past the end of a
    /// slice panics: there is no value it could return.
    pub fn random_range<T: SampleUniform, R: RangeBounds<T>>(&mut self, range: R) -> T {
        T::sample(self, range)
    }

    /// Returns `true` with probability `probability`.
    ///
    /// # Panics
    /// Panics if `probability` is not between 0 and 1.
    #[allow(clippy::manual_assert)]
    pub fn random_bool(&mut self, probability: f64) -> bool {
        // 53 bits is the whole mantissa of an f64, so every probability that can
        // be written down is represented exactly by the comparison below.
        const SCALE: f64 = (1_u64 << 53) as f64;

        assert!((0.0..=1.0).contains(&probability), "a probability has to be between 0 and 1, got {probability}");
        #[allow(clippy::cast_sign_loss)]
        let threshold = (probability * SCALE) as u64;
        (self.next_u64() >> 11) < threshold
    }

    /// Returns a uniformly distributed value in `[0, 1)`.
    pub const fn random_f64(&mut self) -> f64 {
        // The 53 significant bits of an f64, scaled into the unit interval.
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// Shuffles a slice into a uniformly distributed random order.
    pub const fn shuffle<T>(&mut self, slice: &mut [T]) {
        // Fisher-Yates, from the back: element `i` is swapped with a uniformly
        // chosen element of the part that has not been shuffled yet.
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

/// Draws a seed from the operating system.
///
/// `RandomState` is seeded by the platform's randomness and is the only such
/// source in the standard library; the clock is mixed in so that two processes
/// that somehow share a `RandomState` still differ.
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
///
/// Implemented for every primitive integer type; there is no reason for anything
/// outside EZCP to implement it.
pub trait SampleUniform: Sized {
    /// Draws one value from `range`.
    fn sample<R: RangeBounds<Self>>(rng: &mut Rng, range: R) -> Self;
}

/// Implements [`SampleUniform`] for an integer type by widening it to `u64`.
///
/// The distance between two values of any 64-bit-or-smaller integer type fits in
/// a `u64`, so the bounds are mapped onto an offset from `start` and the sampling
/// itself happens in one place.
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

                    // Both casts go through the unsigned type of the same width,
                    // so the subtraction wraps into a plain distance even when the
                    // range spans zero.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_lossless)]
                    let span = (high as u64).wrapping_sub(low as u64);
                    // `span + 1` is the number of values in the range, and it is
                    // zero exactly when the range covers the whole type, which is
                    // the unbounded case `below` treats as "any value".
                    let offset = rng.below(span.wrapping_add(1));
                    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                    { (low as u64).wrapping_add(offset) as Self }
                }
            }
        )*
    };
}

impl_sample_uniform!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

/// Reports a range that contains no values.
///
/// Split out so the sampling macro stays readable, and marked cold because it
/// never returns on any working generator.
#[cold]
#[allow(clippy::panic)]
fn empty_range() -> ! {
    panic!("cannot draw a random value from an empty range");
}
