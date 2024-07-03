pub struct Logger {
    print_to_console: bool,
}

/// A simple println wrapper that can be turned off.
impl Logger {
    pub const fn new(print_to_console: bool) -> Self {
        Self { print_to_console }
    }

    pub fn logln<S: Into<String>>(&self, message: S) {
        let mut message = message.into();
        message.push('\n');
        self.log(message);
    }

    pub fn log<S: Into<String>>(&self, message: S) {
        if self.print_to_console {
            print!("{}", message.into());
        }
    }
}
