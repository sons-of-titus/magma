//! Snippet engine facade.
//!
//! Tab-stop parsing and the built-in snippet table live in the `snippet`
//! crate; this module keeps the editor-facing entry points that the command
//! layer already calls.

pub use snippet::{TabStop, parse_tabstops};

use crate::kernel::state::Editor;

/// Expand a snippet by trigger word.
/// Returns `Some(expanded_text)` if the trigger matches a known snippet, or `None`.
pub fn expand_snippet(_editor: &Editor, trigger: &str) -> Option<String> {
    snippet::expand(trigger)
}

/// Register a new snippet (called from Janet or command line).
pub fn register_snippet(_editor: &mut Editor, _trigger: String, _expansion: String) {
    // In a full implementation, snippets would be stored on Editor state.
}
