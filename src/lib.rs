//! Magma — A programmable text editor.
//! Rust core (kernel) + Janet runtime (userland).

#![allow(unsafe_op_in_unsafe_fn)]

// Debug macros — defined at crate root so `debug!()` works everywhere without import
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::log::enabled() {
            eprintln!("{}", format_args!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! debug_http {
    ($($arg:tt)*) => {
        if $crate::log::is_http() {
            eprintln!("[HTTP] {}", format_args!($($arg)*));
        }
    };
}

pub mod log;
pub mod kernel;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod test_janet_integration;
