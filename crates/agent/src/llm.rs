//! Shared agent/LLM vocabulary: tool schema, system prompt, context
//! shaping, and response-size guards.  Protocol-specific request building
//! and parsing live in `translate`; provider knowledge lives in `registry`
//! and `providers`.

use serde_json::{Value, json};

pub const MAX_BUFFER_CONTEXT_CHARS: usize = 8_000;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

pub fn tools_schema() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "read_active_buffer",
                "description": "Read the text of the currently focused buffer (truncated).",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_diagnostics",
                "description": "Read the current diagnostics (errors/warnings) for the focused buffer.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_editor_commands",
                "description": "List the editor commands available to run_editor_command.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "run_editor_command",
                "description": "Run an editor command, e.g. {\"command\":\"open-file\",\"args\":{\"path\":\"src/main.rs\"}} or {\"command\":\"shell-command\",\"args\":{\"command\":\"cargo build\"}}.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" },
                        "args": {
                            "type": "object",
                            "description": "String-valued command arguments; omit for commands without arguments.",
                            "additionalProperties": { "type": "string" }
                        }
                    },
                    "required": ["command"]
                }
            }
        }
    ])
}

pub fn system_prompt() -> String {
    "You are the Magma editor's built-in agent. You fulfill the user's goal \
     by calling tools — editing files, running commands and build tasks, \
     inspecting diagnostics — and then report concisely what you did and \
     what the result was. Prefer run_editor_command with real editor \
     commands (open-file, save-buffer, shell-command, make, run-config, vc, \
     find-files, keyword-context, goto-line, …); use list_editor_commands to \
     discover more. Mutating commands require the user's approval: the \
     session pauses until they decide, and a declined action reports back \
     to you — never assume it ran. Never fabricate results: if a tool call \
     fails, say so and try a different approach. When the goal is met, \
     answer with a short summary and call no more tools."
        .to_string()
}

pub fn context_message(ctx: &crate::types::AgentContext) -> String {
    format!(
        "Editor context for this session:\n\n{}",
        ctx.to_prompt_string()
    )
}

pub fn truncate_for_context(text: &str) -> String {
    if text.chars().count() <= MAX_BUFFER_CONTEXT_CHARS {
        return text.to_string();
    }
    let cut: String = text.chars().take(MAX_BUFFER_CONTEXT_CHARS).collect();
    format!("{cut}\n…[truncated]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_schema_has_function_definitions() {
        let tools = tools_schema();
        let arr = tools.as_array().unwrap();
        assert!(arr.len() >= 4);
        for t in arr {
            assert_eq!(t["type"], "function");
            assert!(t["function"]["name"].is_string());
            assert!(t["function"]["parameters"].is_object());
        }
    }

    #[test]
    fn truncation_keeps_prefix_and_marks_it() {
        let long = "x".repeat(MAX_BUFFER_CONTEXT_CHARS + 100);
        let cut = truncate_for_context(&long);
        assert!(cut.contains("…[truncated]"));
        assert!(cut.chars().count() < long.chars().count());
        assert_eq!(truncate_for_context("short"), "short");
    }
}
