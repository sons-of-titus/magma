//! Magma — A programmable text editor.
//! Rust core (kernel) + Janet runtime (userland).

#![allow(unsafe_op_in_unsafe_fn)]

// Logging macros (`error!` … `debug!`, `debug_http!`) come from the `logging`
// crate and are imported crate-wide so call sites need no import of their own.
// Output is gated on `MAGMA_LOG` — see that crate's docs.
#[macro_use]
extern crate logging;

pub mod kernel;

#[cfg(test)]
mod tests;
