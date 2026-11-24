//! Snippet engine facade.
//!
//! Tab-stop parsing and the built-in snippet table live in the `snippet`
//! crate; this module keeps the editor-facing entry points that the command
//! layer already calls, backed by the registered-snippet table on editor
//! state (plugins and users contribute; the built-in table stays the
//! always-available fallback).

pub use snippet::{TabStop, expand_lsp};

use crate::state::Editor;

/// The language a buffer's snippets resolve against: its major mode with
/// the `-mode` suffix stripped, or None for non-language buffers.
fn language_of(editor: &Editor) -> Option<String> {
    let buf_id = crate::input::focused_buffer_id(editor);
    editor
        .buffers
        .get(buf_id)
        .and_then(|arc| {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            let name = buf.major_mode.name().to_string();
            name.strip_suffix("-mode").map(|s| s.to_string())
        })
}

/// Expand a trigger: registered snippets first (language-scoped beats
/// global), then the built-in table.
pub fn expand_snippet(editor: &Editor, trigger: &str) -> Option<String> {
    let language = language_of(editor);
    editor
        .snippet
        .registry
        .get(trigger, language.as_deref())
        .map(|s| s.body.clone())
        .or_else(|| snippet::expand(trigger))
}

/// Register a snippet: trigger, LSP-syntax body, optional language and
/// description.
pub fn register_snippet(
    editor: &mut Editor,
    trigger: String,
    body: String,
    language: Option<String>,
    description: Option<String>,
) {
    editor.snippet.registry.register(crate::state::snippet::Snippet {
        trigger,
        body,
        language,
        description,
    });
}
