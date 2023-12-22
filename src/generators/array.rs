use rand::prelude::ThreadRng;
use rand::Rng;

pub fn array_to_string(array: &Vec<i32>, include_count: bool) -> String {
    let mut result = String::new();
    if include_count {
        result.push_str(&format!("{}\n", array.len()));
    }

    for i in 0..array.len() {
        result.push_str(&format!("{} ", array[i]));
    }

    result.push_str("\n");
    result
}

pub fn array_generator_custom(min_n: i32, max_n: i32, gen: impl Fn(&mut ThreadRng) -> i32) -> impl Fn() -> String {
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
