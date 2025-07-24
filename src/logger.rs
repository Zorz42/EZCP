#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugLevel {
    None,
    Basic,
    Detailed,
}

pub struct Logger {
    pub print_to_console: bool,
    pub debug_level: DebugLevel,
}

/// A simple println wrapper that can be turned off.
impl Logger {
    pub const fn new() -> Self {
        Self { 
            print_to_console: true,
            debug_level: DebugLevel::None,
        }
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
