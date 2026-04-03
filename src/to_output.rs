pub trait ToOutput {
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

impl ToOutput for char {
    fn to_output(self) -> String {
        self.to_string()
    }
}

impl ToOutput for bool {
    fn to_output(self) -> String {
        if self { "1".to_owned() } else { "0".to_owned() }
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

impl_to_output!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64);

impl<T: ToOutput> ToOutput for Vec<T> {
    fn to_output(self) -> String {
        if self.is_empty() {
            return String::new();
        }

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

macro_rules! impl_tuple_to_output {
    ( $($name:ident)+ ) => {
        impl<$($name: ToOutput),+> ToOutput for ($($name,)+) {
            #[allow(non_snake_case)]
            fn to_output(self) -> String {
                let ($($name,)+) = self;
                let mut res = String::new();
                $(
                    let s = $name.to_output();
                    res.push_str(&s);
                    if !s.ends_with(|c: char| c.is_whitespace()) {
                        res.push(' ');
                    }
                )+
                if res.ends_with(' ') {
                    res.pop();
                }
                res.push('\n');
                res
            }
        }
    };
}

impl_tuple_to_output! { A }
impl_tuple_to_output! { A B }
impl_tuple_to_output! { A B C }
impl_tuple_to_output! { A B C D }
impl_tuple_to_output! { A B C D E }
impl_tuple_to_output! { A B C D E F }
impl_tuple_to_output! { A B C D E F G }
impl_tuple_to_output! { A B C D E F G H }
impl_tuple_to_output! { A B C D E F G H I }
impl_tuple_to_output! { A B C D E F G H I J }
impl_tuple_to_output! { A B C D E F G H I J K }
impl_tuple_to_output! { A B C D E F G H I J K L }
