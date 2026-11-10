use std::fmt::Write as _;

/// Renders a generated value as the contents of a test input file.
///
/// A `Vec`, slice, array or tuple goes on one line, separated by spaces. The
/// derive writes a struct's fields in order, one per line, skipping fields that
/// render empty:
///
/// ```
/// use ezcp::ToOutput;
///
/// #[derive(ToOutput)]
/// struct Input {
///     n: i32,
///     values: Vec<i32>,
/// }
/// ```
///
/// [`Counted`], [`Lines`] and [`CountedLines`] cover the other usual layouts.
pub trait ToOutput {
    /// Renders `self` as test input.
    fn to_output(self) -> String;

    /// Appends what [`ToOutput::to_output`] returns to `out`.
    ///
    /// The default does exactly that. The implementations in this crate write
    /// straight into `out` instead, which spares a large test one allocation
    /// per value, and an implementation by hand is free to do the same.
    fn write_output(self, out: &mut String)
    where
        Self: Sized,
    {
        out.push_str(&self.to_output());
    }
}

pub use ezcp_macros::ToOutput;

/// `to_output` for a type that implements `write_output` itself.
fn rendered<T: ToOutput>(value: T) -> String {
    let mut out = String::new();
    value.write_output(&mut out);
    out
}

impl ToOutput for String {
    fn to_output(self) -> String {
        self
    }

    fn write_output(self, out: &mut String) {
        out.push_str(&self);
    }
}

impl ToOutput for &str {
    fn to_output(self) -> String {
        self.to_owned()
    }

    fn write_output(self, out: &mut String) {
        out.push_str(self);
    }
}

impl ToOutput for bool {
    fn to_output(self) -> String {
        rendered(self)
    }

    fn write_output(self, out: &mut String) {
        out.push(if self { '1' } else { '0' });
    }
}

impl ToOutput for char {
    fn to_output(self) -> String {
        rendered(self)
    }

    fn write_output(self, out: &mut String) {
        out.push(self);
    }
}

/// Appends a number in decimal. Much of a large test is numbers, and this is
/// several times faster than formatting them.
fn push_decimal(out: &mut String, mut magnitude: u64, negative: bool) {
    // `u64::MAX` has 20 digits.
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
                fn to_output(self) -> String {
                    rendered(self)
                }

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

/// Types too wide for [`push_decimal`], or with a format of their own.
macro_rules! impl_to_output_by_formatting {
    ($($t:ty),*) => {
        $(
            impl ToOutput for $t {
                fn to_output(self) -> String {
                    self.to_string()
                }

                fn write_output(self, out: &mut String) {
                    // Writing to a `String` cannot fail.
                    let _ = write!(out, "{self}");
                }
            }
        )*
    };
}

impl_to_output_by_formatting!(i128, u128, f32, f64);

/// Values on one line, separated by spaces, except that a value which ends its
/// own line is not followed by a space.
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

/// Every value on a line of its own.
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
    fn to_output(self) -> String {
        rendered(self)
    }

    fn write_output(self, out: &mut String) {
        write_sequence(self.into_iter(), out);
    }
}

/// Like a `Vec`. The values are cloned, as rendering consumes them.
impl<T: ToOutput + Clone> ToOutput for &[T] {
    fn to_output(self) -> String {
        rendered(self)
    }

    fn write_output(self, out: &mut String) {
        write_sequence(self.iter().cloned(), out);
    }
}

/// Like a `Vec`.
impl<T: ToOutput, const N: usize> ToOutput for [T; N] {
    fn to_output(self) -> String {
        rendered(self)
    }

    fn write_output(self, out: &mut String) {
        write_sequence(self.into_iter(), out);
    }
}

/// The number of values on one line, then the values as a `Vec` writes them:
/// on one line, or on a line each if they are lines themselves, as tuples and
/// derived structs are.
///
/// ```
/// use ezcp::{Counted, ToOutput};
///
/// assert_eq!(Counted(vec![5, 7, 9]).to_output(), "3\n5 7 9\n");
/// // Queries, edges or whole test cases:
/// assert_eq!(Counted(vec![(1, 2), (3, 4)]).to_output(), "2\n1 2\n3 4\n");
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Counted<T>(pub Vec<T>);

impl<T: ToOutput> ToOutput for Counted<T> {
    fn to_output(self) -> String {
        rendered(self)
    }

    fn write_output(self, out: &mut String) {
        self.0.len().write_output(out);
        out.push('\n');
        self.0.write_output(out);
    }
}

/// Every value on a line of its own.
///
/// ```
/// use ezcp::{Lines, ToOutput};
///
/// assert_eq!(Lines(vec![5, 7, 9]).to_output(), "5\n7\n9\n");
/// assert_eq!(Lines(vec!["ab", "cd"]).to_output(), "ab\ncd\n");
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Lines<T>(pub Vec<T>);

impl<T: ToOutput> ToOutput for Lines<T> {
    fn to_output(self) -> String {
        rendered(self)
    }

    fn write_output(self, out: &mut String) {
        write_lines(self.0.into_iter(), out);
    }
}

/// The number of values on one line, then every value on a line of its own.
///
/// ```
/// use ezcp::{CountedLines, ToOutput};
///
/// assert_eq!(CountedLines(vec![5, 7, 9]).to_output(), "3\n5\n7\n9\n");
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CountedLines<T>(pub Vec<T>);

impl<T: ToOutput> ToOutput for CountedLines<T> {
    fn to_output(self) -> String {
        rendered(self)
    }

    fn write_output(self, out: &mut String) {
        self.0.len().write_output(out);
        out.push('\n');
        write_lines(self.0.into_iter(), out);
    }
}

/// One value of a tuple, followed by a space unless it ends in whitespace.
fn write_tuple_field<T: ToOutput>(value: T, out: &mut String) {
    let start = out.len();
    value.write_output(out);
    if !out[start..].ends_with(char::is_whitespace) {
        out.push(' ');
    }
}

/// Implements `ToOutput` for tuples of every length up to that of the list.
macro_rules! impl_tuple_to_output {
    () => {};
    ( $first:ident $($rest:ident)* ) => {
        impl_tuple_to_output! { $($rest)* }

        impl<$first: ToOutput $(, $rest: ToOutput)*> ToOutput for ($first, $($rest,)*) {
            fn to_output(self) -> String {
                rendered(self)
            }

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
