//! ContextProvider implementations (Phase 11 — AI/Agent Architecture).
//!
//! `EditorContextProvider` — reads live editor state into an `AgentContext`.
//! `LlmContextProvider`    — wraps `EditorContextProvider`; formats context
//!                            as a prompt string for an LLM backend.

use crate::kernel::state::Editor;
use super::types::{AgentContext, ContextProvider};

/// Gathers a snapshot of the current editor state as an `AgentContext`.
pub struct EditorContextProvider;

impl ContextProvider for EditorContextProvider {
    fn gather(&self, ed: &Editor) -> AgentContext {
        let focused_buf_id = ed.view_tree.focused_buffer();

        let (content, path, language) = focused_buf_id
            .and_then(|id| {
                let arc = ed.buffers.get(id)?;
                let buf = arc.lock().unwrap();
                let p = buf.path.clone();
                let len = buf.len();
                let text = buf.slice(0, len.min(4096));
                let lang = ed.ts_languages.get(&id).cloned().unwrap_or_default();
                Some((text, p, lang))
            })
            .unwrap_or_default();

        let (cursor_line, cursor_col) = focused_buf_id
            .and_then(|id| ed.views.get(&id))
            .map(|v| (v.cursor.line, v.cursor.column))
            .unwrap_or((0, 0));

        let diagnostics: Vec<String> = path.as_ref()
            .map(|p| {
                ed.semantic.diagnostics_for_path(p)
                    .into_iter()
                    .map(|d| format!("[{}] {}", d.severity.as_str(), d.message))
                    .collect()
            })
            .unwrap_or_default();

        let symbols: Vec<String> = ed.semantic.symbol_index.all_symbols()
            .into_iter()
            .take(50)
            .map(|s| s.name)
            .collect();

        AgentContext {
            active_buffer_content: content,
            active_buffer_path: path,
            active_buffer_language: language,
            diagnostics,
            symbols,
            project_name: ed.project_manager.current_project.clone(),
            project_language: ed.project_manager.project.language.clone(),
            cursor_line,
            cursor_col,
        }
    }
}

/// Wraps `EditorContextProvider` and formats the gathered context as an LLM prompt.
///
/// The `api_url` and `api_key` fields are metadata for future LLM integrations.
/// The actual HTTP call is left to extension code that subscribes to `agent-thought`
/// and `agent-action` events, keeping the core free of API keys.
pub struct LlmContextProvider {
    pub api_url: String,
    pub api_key: String,
    inner: EditorContextProvider,
}

impl LlmContextProvider {
    pub fn new(api_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            api_url: api_url.into(),
            api_key: api_key.into(),
            inner: EditorContextProvider,
        }
    }

    /// Format `goal` + `ctx` as a prompt string suitable for an LLM chat API.
    pub fn format_for_llm(&self, goal: &str, ctx: &AgentContext) -> String {
        format!(
            "You are an editor assistant.  Help the user accomplish:\n\n{goal}\n\nEditor context:\n\n{}",
            ctx.to_prompt_string()
        )
    }
}

impl ContextProvider for LlmContextProvider {
    fn gather(&self, ed: &Editor) -> AgentContext {
        self.inner.gather(ed)
    }
}
