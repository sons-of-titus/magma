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

pub mod state;
pub mod buffer;
pub mod net;
pub mod dired;
pub mod vc;
pub mod clipboard;
pub mod window;
pub mod event;
pub mod command;
pub mod keymap;
pub mod input;
pub mod fs;
pub mod render;
pub mod lsp;
pub mod process;
pub mod runtime;
pub mod server;
pub mod terminal;
pub mod snippet;
pub mod log;
pub mod scripting;

#[cfg(feature = "janet")]
pub mod janet_bridge;

#[cfg(test)]
mod tests;
