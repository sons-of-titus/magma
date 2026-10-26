//! Debug Adapter Protocol model: breakpoints, stack frames, variables,
//! threads, and the mutable state of one adapter session.
//!
//! This crate is pure state — no processes, no channels, no editor.  The
//! DAP client that spawns adapters and the manager that orchestrates
//! sessions stay in the kernel.

#[cfg(test)]
mod tests;

pub mod session;
pub mod types;

pub use session::DebugSession;
pub use types::{Breakpoint, FrameId, MemoryRegion, SessionId, StackFrame, Thread, ThreadId, Variable};
