use rand::prelude::ThreadRng;
use rand::Rng;
use std::fmt::Write;

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

pub fn array_generator_custom<F>(min_n: i32, max_n: i32, gen: F) -> impl Fn() -> String
where
    F: Fn(&mut ThreadRng) -> i32,
{
    move || {
        let mut rng = rand::thread_rng();
        let n = rng.gen_range(min_n..=max_n);
        let mut array = Vec::new();
        for _ in 0..n {
            array.push(gen(&mut rng));
        }
        array_to_string(&array, true)
    }
}

pub fn array_generator(min_n: i32, max_n: i32, min_x: i32, max_x: i32) -> impl Fn() -> String {
    array_generator_custom(min_n, max_n, move |rng| rng.gen_range(min_x..=max_x))
}
