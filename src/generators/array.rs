use crate::rng::Rng;
use std::fmt::Write;

/// Formats `[1, 2, 3]` as `"3\n1 2 3\n"`, or as `"1 2 3\n"` without the count.
#[must_use]
pub fn array_to_string(array: &[i32], include_count: bool) -> String {
    let mut result = String::new();
    if include_count {
        writeln!(result, "{}", array.len()).ok();
    }

    for (idx, value) in array.iter().enumerate() {
        if idx > 0 {
            result.push(' ');
        }
        write!(result, "{value}").ok();
    }

    result.push('\n');
    result
}

/// Returns a generator of arrays, with the count, of `min_n..=max_n` elements
/// drawn by `generator`.
pub fn array_generator_custom<F: Fn(&mut Rng) -> i32>(min_n: i32, max_n: i32, generator: F) -> impl Fn(&mut Rng) -> String {
    move |rng| {
        let n = rng.random_range(min_n..=max_n);
        let mut array = Vec::new();
        for _ in 0..n {
            array.push(generator(rng));
        }
        array_to_string(&array, true)
    }
}

/// Returns a generator of arrays, with the count, of `min_n..=max_n` elements
/// in `min_x..=max_x`.
pub fn array_generator(min_n: i32, max_n: i32, min_x: i32, max_x: i32) -> impl Fn(&mut Rng) -> String {
    array_generator_custom(min_n, max_n, move |rng| rng.random_range(min_x..=max_x))
}
