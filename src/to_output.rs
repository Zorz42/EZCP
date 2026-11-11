use std::fmt::Write as _;

/// How a value is written into a test file. Implement `write_output`; `to_output` follows from it.
pub trait ToOutput: Sized {
    fn write_output(self, out: &mut String);

    fn to_output(self) -> String {
        let mut out = String::new();
        self.write_output(&mut out);
        out
    }
}

pub use ezcp_macros::ToOutput;

impl ToOutput for String {
    fn write_output(self, out: &mut String) {
        out.push_str(&self);
    }

    fn to_output(self) -> String {
        self
    }
}

impl ToOutput for &str {
    fn write_output(self, out: &mut String) {
        out.push_str(self);
    }
}

impl ToOutput for bool {
    fn write_output(self, out: &mut String) {
        out.push(if self { '1' } else { '0' });
    }
}

impl ToOutput for char {
    fn write_output(self, out: &mut String) {
        out.push(self);
    }
}

/// Several times faster than `write!`, and much of a large test is numbers.
fn push_decimal(out: &mut String, mut magnitude: u64, negative: bool) {
    let mut digits = [0_u8; 20];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + (magnitude % 10) as u8;
        magnitude /= 10;
        if magnitude == 0 {
            break;
        }
    }
    if negative {
        out.push('-');
    }
    out.extend(digits[start..].iter().map(|&digit| char::from(digit)));
}

macro_rules! impl_to_output_for_integers {
    ($($int:ty),* => |$value:ident| $magnitude:expr, $negative:expr) => {
        $(
            impl ToOutput for $int {
                #[allow(clippy::unnecessary_cast)]
                fn write_output(self, out: &mut String) {
                    let $value = self;
                    push_decimal(out, $magnitude, $negative);
                }
            }
        )*
    };
}

impl_to_output_for_integers!(i8, i16, i32, i64, isize => |value| value.unsigned_abs() as u64, value < 0);
impl_to_output_for_integers!(u8, u16, u32, u64, usize => |value| value as u64, false);

macro_rules! impl_to_output_by_formatting {
    ($($t:ty),*) => {
        $(
            impl ToOutput for $t {
                fn write_output(self, out: &mut String) {
                    let _ = write!(out, "{self}");
                }
            }
        )*
    };
}

impl_to_output_by_formatting!(i128, u128, f32, f64);

/// Space-separated on one line, except that a value ending its own line is not followed by a space.
fn write_sequence<T: ToOutput, I: Iterator<Item = T>>(values: I, out: &mut String) {
    let start = out.len();
    for value in values {
        let value_start = out.len();
        value.write_output(out);
        if !out[value_start..].ends_with('\n') {
            out.push(' ');
        }
    }
    if out[start..].ends_with(' ') {
        out.pop();
        out.push('\n');
    }
}

fn write_lines<T: ToOutput, I: Iterator<Item = T>>(values: I, out: &mut String) {
    for value in values {
        let value_start = out.len();
        value.write_output(out);
        if !out[value_start..].ends_with('\n') {
            out.push('\n');
        }
    }
}

impl<T: ToOutput> ToOutput for Vec<T> {
    fn write_output(self, out: &mut String) {
        write_sequence(self.into_iter(), out);
    }
}

impl<T: ToOutput + Clone> ToOutput for &[T] {
    fn write_output(self, out: &mut String) {
        write_sequence(self.iter().cloned(), out);
    }
}

impl<T: ToOutput, const N: usize> ToOutput for [T; N] {
    fn write_output(self, out: &mut String) {
        write_sequence(self.into_iter(), out);
    }
}

/// The length on one line, then the values as a `Vec` writes them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Counted<T>(pub Vec<T>);

impl<T: ToOutput> ToOutput for Counted<T> {
    fn write_output(self, out: &mut String) {
        self.0.len().write_output(out);
        out.push('\n');
        self.0.write_output(out);
    }
}

/// One value per line.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Lines<T>(pub Vec<T>);

impl<T: ToOutput> ToOutput for Lines<T> {
    fn write_output(self, out: &mut String) {
        write_lines(self.0.into_iter(), out);
    }
}

/// The length on one line, then one value per line.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CountedLines<T>(pub Vec<T>);

impl<T: ToOutput> ToOutput for CountedLines<T> {
    fn write_output(self, out: &mut String) {
        self.0.len().write_output(out);
        out.push('\n');
        write_lines(self.0.into_iter(), out);
    }
}

fn write_tuple_field<T: ToOutput>(value: T, out: &mut String) {
    let start = out.len();
    value.write_output(out);
    if !out[start..].ends_with(char::is_whitespace) {
        out.push(' ');
    }
}

macro_rules! impl_tuple_to_output {
    () => {};
    ( $first:ident $($rest:ident)* ) => {
        impl_tuple_to_output! { $($rest)* }

        impl<$first: ToOutput $(, $rest: ToOutput)*> ToOutput for ($first, $($rest,)*) {
            #[allow(non_snake_case)]
            fn write_output(self, out: &mut String) {
                let ($first, $($rest,)*) = self;
                let start = out.len();
                write_tuple_field($first, out);
                $(write_tuple_field($rest, out);)*
                if out[start..].ends_with(' ') {
                    out.pop();
                }
                if !out[start..].ends_with('\n') {
                    out.push('\n');
                }
            }
        }
    };
}

impl_tuple_to_output! { A B C D E F G H I J K L }
