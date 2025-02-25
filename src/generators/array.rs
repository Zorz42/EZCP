use rand::prelude::ThreadRng;
use std::fmt::Write;
use rand::Rng;

/// This function converts an array of integers to a string.
/// It is used to generate the input for test cases.
/// If it receives an array of [1, 2, 3] and `include_count` is `true`, it will return the string "3\n1 2 3\n".
/// If `include_count` is `false`, it will return the string "1 2 3\n".
#[must_use]
pub fn array_to_string(array: &Vec<i32>, include_count: bool) -> String {
    let mut result = String::new();
    if include_count {
        writeln!(result, "{}", array.len()).ok();
    }

    for i in array {
        write!(result, "{i} ").ok();
    }
    
    result.push('\n');
    result
}

/// This function returns a function that generates an array of integers with a custom generator.
/// The array will have a length between `min_n` and `max_n` (inclusive).
/// The generator function will be called to generate each element in the array.
pub fn array_generator_custom<F: Fn(&mut ThreadRng) -> i32>(min_n: i32, max_n: i32, gen: F) -> impl Fn() -> String {
    move || {
        let mut rng = rand::rng();
        let n = rng.gen_range(min_n..=max_n);
        let mut array = Vec::new();
        for _ in 0..n {
            array.push(gen(&mut rng));
        }
        array_to_string(&array, true)
    }
}

/// This function returns a function that generates an array of integers.
/// The array will have a length between `min_n` and `max_n` (inclusive).
/// The values in the array will be between `min_x` and `max_x` (inclusive).
pub fn array_generator(min_n: i32, max_n: i32, min_x: i32, max_x: i32) -> impl Fn() -> String {
    array_generator_custom(min_n, max_n, move |rng| rng.gen_range(min_x..=max_x))
}
