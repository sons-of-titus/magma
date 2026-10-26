//! VT/ANSI terminal emulation.
//!
//! A self-contained screen model (`TerminalScreen`: cells, styles, cursor,
//! scrollback) and the escape-sequence parser that drives it (`ansi::feed`).
//! No PTY, no editor state, no background runtime — the kernel's terminal
//! subsystem owns sessions and feeds bytes into this crate.

pub mod screen;
pub mod ansi;

pub use ansi::feed;
pub use screen::{Cell, Color, Span, Style, StyledLine, TerminalScreen};
