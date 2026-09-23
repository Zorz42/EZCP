/// Renders a generated value as the contents of a test input file.
///
/// A `Vec` or tuple goes on one line, separated by spaces. The derive writes a
/// struct's fields in order, one per line, skipping fields that render empty:
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
pub trait ToOutput {
    /// Renders `self` as test input.
    fn to_output(self) -> String;
}

pub use ezcp_macros::ToOutput;

impl ToOutput for String {
    fn to_output(self) -> String {
        self
    }
}

impl ToOutput for &str {
    fn to_output(self) -> String {
        self.to_owned()
    }
}

impl ToOutput for bool {
    fn to_output(self) -> String {
        u8::from(self).to_string()
    }
}

macro_rules! impl_to_output {
    ($($t:ty),*) => {
        $(
            impl ToOutput for $t {
                fn to_output(self) -> String {
                    self.to_string()
                }
            }
        )*
    };
}

impl_to_output!(char, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64);

impl<T: ToOutput> ToOutput for Vec<T> {
    fn to_output(self) -> String {
        let mut res = String::new();
        for i in self {
            let s = i.to_output();
            res.push_str(&s);
            if !s.ends_with('\n') {
                res.push(' ');
            }
        }
        if res.ends_with(' ') {
            res.pop();
            res.push('\n');
        }
        res
    }
}

/// Implements `ToOutput` for tuples of every length up to that of the list.
macro_rules! impl_tuple_to_output {
    () => {};
    ( $first:ident $($rest:ident)* ) => {
        impl_tuple_to_output! { $($rest)* }

        impl<$first: ToOutput $(, $rest: ToOutput)*> ToOutput for ($first, $($rest,)*) {
            #[allow(non_snake_case)]
            fn to_output(self) -> String {
                let ($first, $($rest,)*) = self;
                let mut res = String::new();
                for s in [$first.to_output() $(, $rest.to_output())*] {
                    res.push_str(&s);
                    if !s.ends_with(char::is_whitespace) {
                        res.push(' ');
                    }
                }
                if res.ends_with(' ') {
                    res.pop();
                }
                if !res.ends_with('\n') {
                    res.push('\n');
                }
                res
            }
        }
    };
}

impl_tuple_to_output! { A B C D E F G H I J K L }
