//! xoshiro256++ seeded through `SplitMix64`, with Lemire's bounded sampling. Hand-written because a
//! seed has to rebuild the same test forever, which no crate promises: nothing here may change the stream.

use std::collections::HashSet;
use std::hash::{BuildHasher as _, Hasher as _, RandomState};
use std::ops::{Bound, RangeBounds};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: [u64; 4],
}

const fn split_mix_64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    #[must_use]
    pub const fn from_seed(seed: u64) -> Self {
        let mut mixer = seed;
        let state = [split_mix_64(&mut mixer), split_mix_64(&mut mixer), split_mix_64(&mut mixer), split_mix_64(&mut mixer)];
        Self { state }
    }

    #[must_use]
    pub fn from_entropy() -> Self {
        Self::from_seed(random_seed())
    }

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

    /// Uniform below `bound`; a `bound` of zero means any `u64`.
    const fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return self.next_u64();
        }

        let mut product = (self.next_u64() as u128).wrapping_mul(bound as u128);
        let mut low = product as u64;
        if low < bound {
            let threshold = bound.wrapping_neg() % bound;
            while low < threshold {
                product = (self.next_u64() as u128).wrapping_mul(bound as u128);
                low = product as u64;
            }
        }
        (product >> 64) as u64
    }

    pub fn random_range<T: SampleUniform, R: RangeBounds<T>>(&mut self, range: R) -> T {
        T::sample(self, range)
    }

    pub fn random_bool(&mut self, probability: f64) -> bool {
        // 53 bits, so every f64 probability is exact.
        const SCALE: f64 = (1_u64 << 53) as f64;

        assert!((0.0..=1.0).contains(&probability), "a probability has to be between 0 and 1, got {probability}");
        let threshold = (probability * SCALE) as u64;
        (self.next_u64() >> 11) < threshold
    }

    /// Uniform in `[0, 1)`.
    pub const fn random_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    pub const fn shuffle<T>(&mut self, slice: &mut [T]) {
        let mut i = slice.len();
        while i > 1 {
            i -= 1;
            let j = self.below(i as u64 + 1) as usize;
            slice.swap(i, j);
        }
    }

    pub const fn choose<'slice, T>(&mut self, slice: &'slice [T]) -> Option<&'slice T> {
        if slice.is_empty() {
            return None;
        }
        let idx = self.below(slice.len() as u64) as usize;
        Some(&slice[idx])
    }

    pub fn random_vec<T: SampleUniform, R: RangeBounds<T> + Clone>(&mut self, len: usize, range: R) -> Vec<T> {
        (0..len).map(|_| self.random_range(range.clone())).collect()
    }

    /// Two values from `range`, the smaller first.
    pub fn random_interval<T: SampleUniform + PartialOrd, R: RangeBounds<T> + Clone>(&mut self, range: R) -> (T, T) {
        let (first, second) = (self.random_range(range.clone()), self.random_range(range));
        if second < first { (second, first) } else { (first, second) }
    }

    /// The largest of `skew + 1` draws, or for a negative `skew` the smallest of `1 - skew`.
    pub fn random_range_skewed<T: SampleUniform + PartialOrd, R: RangeBounds<T> + Clone>(&mut self, range: R, skew: i32) -> T {
        let mut best = self.random_range(range.clone());
        for _ in 0..skew.unsigned_abs() {
            let other = self.random_range(range.clone());
            if (skew > 0 && other > best) || (skew < 0 && other < best) {
                best = other;
            }
        }
        best
    }

    pub fn shuffled<I: IntoIterator>(&mut self, items: I) -> Vec<I::Item> {
        let mut items: Vec<_> = items.into_iter().collect();
        self.shuffle(&mut items);
        items
    }

    /// A random order of `1..=n`.
    pub fn permutation(&mut self, n: usize) -> Vec<usize> {
        self.shuffled(1..=n)
    }

    /// `count` different values below `total`, every set equally likely (Floyd's algorithm).
    fn distinct_below(&mut self, count: usize, total: u128) -> Vec<u64> {
        let mut picked = Vec::with_capacity(count);
        let mut seen = HashSet::with_capacity(count);
        for limit in total - count as u128..total {
            // `limit + 1` wraps to zero ("any value") once it is all of `u64`.
            let candidate = self.below((limit + 1) as u64);
            // `limit` cannot have come up yet.
            let value = if seen.insert(candidate) { candidate } else { limit as u64 };
            seen.insert(value);
            picked.push(value);
        }
        picked
    }

    /// `count` different values from `range` in random order, in time proportional to `count`.
    pub fn distinct<T: SampleInteger, R: RangeBounds<T>>(&mut self, count: usize, range: R) -> Vec<T> {
        let (low, high) = T::inclusive_bounds(&range);
        let total = (high.to_i128() - low.to_i128()) as u128 + 1;
        assert!(count as u128 <= total, "cannot draw {count} different values from a range of {total}");
        let mut values: Vec<T> = self.distinct_below(count, total).into_iter().map(|offset| T::from_i128(low.to_i128() + offset as i128)).collect();
        self.shuffle(&mut values);
        values
    }

    /// `parts` values of at least `min_part` adding up to `total`, uniformly among all such splits.
    pub fn partition<T: SampleInteger>(&mut self, total: T, parts: usize, min_part: T) -> Vec<T> {
        let spare = total.to_i128() - parts as i128 * min_part.to_i128();
        assert!(
            spare >= 0 && (parts > 0 || spare == 0),
            "cannot split {} into {parts} parts of at least {} each",
            total.to_i128(),
            min_part.to_i128()
        );
        if parts == 0 {
            return Vec::new();
        }

        // Stars and bars: the spare units and `parts - 1` borders stand in a row, and the borders are picked.
        let positions = spare as u128 + parts as u128 - 1;
        let mut borders = self.distinct_below(parts - 1, positions);
        borders.sort_unstable();
        let mut start = 0;
        borders
            .into_iter()
            .map(u128::from)
            .chain([positions])
            .map(|border| {
                let size = border - start;
                start = border + 1;
                T::from_i128(min_part.to_i128() + size as i128)
            })
            .collect()
    }

    pub fn random_string(&mut self, len: usize, alphabet: &str) -> String {
        let letters: Vec<char> = alphabet.chars().collect();
        assert!(!letters.is_empty(), "cannot build a string from an empty alphabet");
        (0..len).map(|_| letters[self.below(letters.len() as u64) as usize]).collect()
    }

    #[allow(clippy::expect_used)]
    pub fn weighted_index(&mut self, weights: &[u64]) -> usize {
        let total = weights
            .iter()
            .try_fold(0_u64, |sum, &weight| sum.checked_add(weight))
            .expect("the weights add up to more than a u64 holds");
        assert!(total > 0, "cannot pick from weights that add up to zero");
        let mut ticket = self.below(total);
        weights
            .iter()
            .position(|&weight| {
                let won = ticket < weight;
                ticket = ticket.wrapping_sub(weight);
                won
            })
            .unwrap_or(0)
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

pub trait SampleUniform: Sized {
    fn sample<R: RangeBounds<Self>>(rng: &mut Rng, range: R) -> Self;
}

pub trait SampleInteger: SampleUniform + Copy {
    #[doc(hidden)]
    fn inclusive_bounds<R: RangeBounds<Self>>(range: &R) -> (Self, Self);
    #[doc(hidden)]
    fn to_i128(self) -> i128;
    #[doc(hidden)]
    fn from_i128(value: i128) -> Self;
}

macro_rules! impl_sample_uniform {
    ($($int:ty),*) => {
        $(
            impl SampleUniform for $int {
                fn sample<R: RangeBounds<Self>>(rng: &mut Rng, range: R) -> Self {
                    let (low, high) = Self::inclusive_bounds(&range);
                    // In u64, where the distance between any two values fits; wrapping covers ranges across zero.
                    let span = (high as u64).wrapping_sub(low as u64);
                    // Wraps to zero ("any value") for all of `u64`.
                    let offset = rng.below(span.wrapping_add(1));
                    (low as u64).wrapping_add(offset) as Self
                }
            }

            impl SampleInteger for $int {
                fn inclusive_bounds<R: RangeBounds<Self>>(range: &R) -> (Self, Self) {
                    let low = match range.start_bound() {
                        Bound::Included(&low) => low,
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
                    (low, high)
                }

                fn to_i128(self) -> i128 {
                    self as i128
                }

                fn from_i128(value: i128) -> Self {
                    value as Self
                }
            }
        )*
    };
}

impl_sample_uniform!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

macro_rules! impl_sample_uniform_for_floats {
    ($($float:ty),*) => {
        $(
            impl SampleUniform for $float {
                fn sample<R: RangeBounds<Self>>(rng: &mut Rng, range: R) -> Self {
                    let (low, (high, inclusive)) = match (range.start_bound(), range.end_bound()) {
                        (Bound::Included(&low) | Bound::Excluded(&low), Bound::Included(&high)) => (low, (high, true)),
                        (Bound::Included(&low) | Bound::Excluded(&low), Bound::Excluded(&high)) => (low, (high, false)),
                        _ => unbounded_float_range(),
                    };
                    if !low.is_finite() || !high.is_finite() {
                        unbounded_float_range();
                    }
                    let has_values = if inclusive { low <= high } else { low < high };
                    if !has_values {
                        empty_range();
                    }

                    let value = (f64::from(low) + (f64::from(high) - f64::from(low)) * rng.random_f64()) as Self;
                    // Rounding can reach the end of the range.
                    if value < high {
                        value
                    } else if inclusive {
                        high
                    } else {
                        low
                    }
                }
            }
        )*
    };
}

impl_sample_uniform_for_floats!(f32, f64);

const SURROGATES: std::ops::Range<u32> = 0xD800..0xE000;

/// Characters are numbered without the surrogate gap.
const fn char_index(character: char) -> u32 {
    let code = character as u32;
    if code >= SURROGATES.end { code - (SURROGATES.end - SURROGATES.start) } else { code }
}

impl SampleUniform for char {
    fn sample<R: RangeBounds<Self>>(rng: &mut Rng, range: R) -> Self {
        let low = match range.start_bound() {
            Bound::Included(&low) => char_index(low),
            Bound::Excluded(&low) => char_index(low) + 1,
            Bound::Unbounded => 0,
        };
        let high = match range.end_bound() {
            Bound::Included(&high) => char_index(high),
            Bound::Excluded(&high) => char_index(high).checked_sub(1).unwrap_or_else(|| empty_range()),
            Bound::Unbounded => char_index(Self::MAX),
        };
        if low > high {
            empty_range();
        }

        let index = low + rng.below(u64::from(high - low) + 1) as u32;
        let code = if index >= SURROGATES.start { index + (SURROGATES.end - SURROGATES.start) } else { index };
        Self::from_u32(code).unwrap_or_else(|| empty_range())
    }
}

#[cold]
#[allow(clippy::panic)]
fn empty_range() -> ! {
    panic!("cannot draw a random value from an empty range");
}

#[cold]
#[allow(clippy::panic)]
fn unbounded_float_range() -> ! {
    panic!("a range of floats to draw from needs two finite ends");
}
