//! Magma — A programmable text editor.
//! Rust core (kernel) + Janet runtime (userland).

#![allow(unsafe_op_in_unsafe_fn)]

pub use kernel;

#[cfg(test)]
mod tests;
