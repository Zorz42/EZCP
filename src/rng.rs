//! xoshiro256++ seeded through `SplitMix64`, with Lemire's bounded sampling.
//!
//! Implemented here rather than taken from a crate because published seeds must
//! rebuild the same tests forever, and crates only keep values stable within a
//! major version. Nothing here may change the stream of numbers.

use std::collections::HashSet;
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

    /// Returns `len` values from `range`.
    ///
    /// ```
    /// # use ezcp::Rng;
    /// let mut rng = Rng::from_seed(1);
    /// let values: Vec<i64> = rng.random_vec(5, 1..=1_000_000_000);
    /// assert!(values.len() == 5 && values.iter().all(|value| (1..=1_000_000_000).contains(value)));
    /// let letters: String = rng.random_vec(5, 'a'..='z').into_iter().collect();
    /// ```
    ///
    /// # Panics
    /// Panics if the range is empty.
    pub fn random_vec<T: SampleUniform, R: RangeBounds<T> + Clone>(&mut self, len: usize, range: R) -> Vec<T> {
        (0..len).map(|_| self.random_range(range.clone())).collect()
    }

    /// Returns two values from `range`, the smaller one first: the ends of a
    /// random interval, as for a query.
    ///
    /// ```
    /// # use ezcp::Rng;
    /// let (left, right) = Rng::from_seed(1).random_interval(1..=100);
    /// assert!(1 <= left && left <= right && right <= 100);
    /// ```
    ///
    /// # Panics
    /// Panics if the range is empty.
    pub fn random_interval<T: SampleUniform + PartialOrd, R: RangeBounds<T> + Clone>(&mut self, range: R) -> (T, T) {
        let (first, second) = (self.random_range(range.clone()), self.random_range(range));
        if second < first { (second, first) } else { (first, second) }
    }

    /// Returns a value from `range` that leans towards one end of it: the
    /// largest of `skew + 1` uniform values if `skew` is positive, the smallest
    /// of `1 - skew` if it is negative. A skew of zero is [`Rng::random_range`].
    ///
    /// Tests near the limits of a task are found far more often this way.
    ///
    /// ```
    /// # use ezcp::Rng;
    /// let mut rng = Rng::from_seed(1);
    /// let mostly_large = rng.random_range_skewed(1..=1000, 3);
    /// let mostly_small = rng.random_range_skewed(1..=1000, -3);
    /// ```
    ///
    /// # Panics
    /// Panics if the range is empty.
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

    /// Returns `items` in random order.
    ///
    /// ```
    /// # use ezcp::Rng;
    /// let mut order = Rng::from_seed(1).shuffled(0..5);
    /// order.sort_unstable();
    /// assert_eq!(order, [0, 1, 2, 3, 4]);
    /// ```
    pub fn shuffled<I: IntoIterator>(&mut self, items: I) -> Vec<I::Item> {
        let mut items: Vec<_> = items.into_iter().collect();
        self.shuffle(&mut items);
        items
    }

    /// Returns a random permutation of `1..=n`. For one of `0..n`, use
    /// [`rng.shuffled(0..n)`](Rng::shuffled).
    ///
    /// ```
    /// # use ezcp::Rng;
    /// let mut permutation = Rng::from_seed(1).permutation(4);
    /// permutation.sort_unstable();
    /// assert_eq!(permutation, [1, 2, 3, 4]);
    /// ```
    pub fn permutation(&mut self, n: usize) -> Vec<usize> {
        self.shuffled(1..=n)
    }

    /// Picks `count` different numbers below `total`, in no particular order
    /// and each set of them equally likely (Floyd's algorithm).
    fn distinct_below(&mut self, count: usize, total: u128) -> Vec<u64> {
        let mut picked = Vec::with_capacity(count);
        let mut seen = HashSet::with_capacity(count);
        for limit in total - count as u128..total {
            // Wraps to zero, meaning "any value", once `limit` is all of `u64`.
            let candidate = self.below((limit + 1) as u64);
            // A repeated candidate is replaced by `limit`, which cannot have come up yet.
            let value = if seen.insert(candidate) { candidate } else { limit as u64 };
            seen.insert(value);
            picked.push(value);
        }
        picked
    }

    /// Returns `count` different values from `range`, in random order.
    ///
    /// It takes time in proportion to `count` however large the range is.
    ///
    /// ```
    /// # use ezcp::Rng;
    /// let mut values = Rng::from_seed(1).distinct(3, 1..=1_000_000_000_i64);
    /// values.sort_unstable();
    /// values.dedup();
    /// assert_eq!(values.len(), 3);
    /// ```
    ///
    /// # Panics
    /// Panics if the range has fewer than `count` values.
    pub fn distinct<T: SampleInteger, R: RangeBounds<T>>(&mut self, count: usize, range: R) -> Vec<T> {
        let (low, high) = T::inclusive_bounds(&range);
        let total = (high.to_i128() - low.to_i128()) as u128 + 1;
        assert!(count as u128 <= total, "cannot draw {count} different values from a range of {total}");
        let mut values: Vec<T> = self.distinct_below(count, total).into_iter().map(|offset| T::from_i128(low.to_i128() + offset as i128)).collect();
        self.shuffle(&mut values);
        values
    }

    /// Splits `total` into `parts` numbers of at least `min_part` each, in one
    /// of all the ways to do so, each equally likely.
    ///
    /// This is how to pick the sizes of the test cases in an input whose sizes
    /// have to add up to a limit.
    ///
    /// ```
    /// # use ezcp::Rng;
    /// let sizes = Rng::from_seed(1).partition(200_000_usize, 10, 1);
    /// assert_eq!(sizes.len(), 10);
    /// assert_eq!(sizes.iter().sum::<usize>(), 200_000);
    /// assert!(sizes.iter().all(|&size| size >= 1));
    /// ```
    ///
    /// # Panics
    /// Panics if `parts * min_part` is more than `total`, or if there are no
    /// parts to hold a `total` other than zero.
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

        // The spare units and the `parts - 1` borders between parts stand in a
        // row; the positions of the borders are what is picked.
        let positions = spare as u128 + parts as u128 - 1;
        let mut borders = self.distinct_below(parts - 1, positions);
        borders.sort_unstable();
        let mut start = 0;
        borders
            .into_iter()
            .map(u128::from)
            // The end of the row closes the last part.
            .chain([positions])
            .map(|border| {
                let size = border - start;
                start = border + 1;
                T::from_i128(min_part.to_i128() + size as i128)
            })
            .collect()
    }

    /// Returns a string of `len` characters, each picked from `alphabet`.
    ///
    /// ```
    /// # use ezcp::Rng;
    /// let dna = Rng::from_seed(1).random_string(8, "ACGT");
    /// assert!(dna.len() == 8 && dna.chars().all(|base| "ACGT".contains(base)));
    /// ```
    ///
    /// # Panics
    /// Panics if `alphabet` is empty.
    pub fn random_string(&mut self, len: usize, alphabet: &str) -> String {
        let letters: Vec<char> = alphabet.chars().collect();
        assert!(!letters.is_empty(), "cannot build a string from an empty alphabet");
        (0..len).map(|_| letters[self.below(letters.len() as u64) as usize]).collect()
    }

    /// Returns an index into `weights`, each picked with a probability in
    /// proportion to its weight.
    ///
    /// ```
    /// # use ezcp::Rng;
    /// let mut rng = Rng::from_seed(1);
    /// // A path one time in ten, a star three times, and otherwise a random tree.
    /// let shape = ["path", "star", "random"][rng.weighted_index(&[1, 3, 6])];
    /// ```
    ///
    /// # Panics
    /// Panics if the weights add up to zero, or to more than a `u64` holds.
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

/// A type that [`Rng::random_range`] can produce: the integers up to 64 bits,
/// `f32`, `f64` and `char`.
pub trait SampleUniform: Sized {
    /// Draws one value from `range`.
    fn sample<R: RangeBounds<Self>>(rng: &mut Rng, range: R) -> Self;
}

/// An integer type, for the methods of [`Rng`] that have to count the values
/// of a range.
pub trait SampleInteger: SampleUniform + Copy {
    /// The smallest and the largest value of `range`.
    ///
    /// # Panics
    /// Panics if the range is empty.
    #[doc(hidden)]
    fn inclusive_bounds<R: RangeBounds<Self>>(range: &R) -> (Self, Self);

    #[doc(hidden)]
    fn to_i128(self) -> i128;

    /// Truncates a value that does not fit.
    #[doc(hidden)]
    fn from_i128(value: i128) -> Self;
}

/// Samples an offset from `low` in `u64`, where the distance between any two
/// values of these types fits.
macro_rules! impl_sample_uniform {
    ($($int:ty),*) => {
        $(
            impl SampleUniform for $int {
                fn sample<R: RangeBounds<Self>>(rng: &mut Rng, range: R) -> Self {
                    let (low, high) = Self::inclusive_bounds(&range);
                    // Wrapping, so it is the distance even when the range spans zero.
                    let span = (high as u64).wrapping_sub(low as u64);
                    // Wraps to zero, meaning "any value", for a range covering all of `u64`.
                    let offset = rng.below(span.wrapping_add(1));
                    (low as u64).wrapping_add(offset) as Self
                }
            }

            impl SampleInteger for $int {
                fn inclusive_bounds<R: RangeBounds<Self>>(range: &R) -> (Self, Self) {
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

/// A value between two finite bounds. The end of the range is left out or kept
/// as the range says, though for floats that hardly changes anything.
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

/// The place of a character among all characters, which are the code points
/// other than the 0x800 surrogates from 0xD800 on.
const SURROGATES: std::ops::Range<u32> = 0xD800..0xE000;

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
