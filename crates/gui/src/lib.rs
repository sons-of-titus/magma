//! Magma's graphical frontend.
//!
//! The crate owns every presentation concern: the shell chrome, input
//! translation, dialogs, and the wgpu/egui editor-pane paint path.  The
//! kernel exposes state, commands, and `render_frame`, which turns editor
//! semantics into a character grid; nothing here is known to the kernel.

pub mod render;

#[cfg(test)]
mod tests;
