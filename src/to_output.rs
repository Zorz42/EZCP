pub trait ToOutput {
    fn to_output(self) -> String;
}

impl ToOutput for String {
    fn to_output(self) -> String {
       self
    }
}

impl ToOutput for i8 {
    fn to_output(self) -> String {
        self.to_string()
    }
}

impl ToOutput for i16 {
    fn to_output(self) -> String {
        self.to_string()
    }
}

impl ToOutput for i32 {
    fn to_output(self) -> String {
        self.to_string()
    }
}

impl ToOutput for i64 {
    fn to_output(self) -> String {
        self.to_string()
    }
}

impl ToOutput for f32 {
    fn to_output(self) -> String {
        self.to_string()
    }
}

impl ToOutput for f64 {
    fn to_output(self) -> String {
        self.to_string()
    }
}

impl ToOutput for u8 {
    fn to_output(self) -> String {
        self.to_string()
    }
}

impl ToOutput for u16 {
    fn to_output(self) -> String {
        self.to_string()
    }
}

impl ToOutput for u32 {
    fn to_output(self) -> String {
        self.to_string()
    }
}

impl ToOutput for u64 {
    fn to_output(self) -> String {
        self.to_string()
    }
}
