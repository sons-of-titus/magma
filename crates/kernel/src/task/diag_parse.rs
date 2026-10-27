//! Tool output → diagnostics; the parser lives in the `tool_diag` crate.
//!
//! Applying findings to buffers and the quickfix list is in this module's
//! parent (`apply_tool_diagnostics`).

pub use tool_diag::{ToolDiag, parse_tool_output};
