//! Common utilities for tests

pub const ORANGE: &str = "\x1b[38;5;214m";
pub const RESET: &str = "\x1b[0m";

/// Provides a macro for logging messages during tests.
/// e.g. log!("placeholder") -> println!("[test] placeholder");
#[macro_export]
macro_rules! log {
    ($msg:expr, $($arg:tt)*) => {
        println!("{}[test] {}{}", crate::common::ORANGE, format!($msg, $($arg)*), crate::common::RESET)
    };
}