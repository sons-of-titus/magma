//! LLM backend for the agent loop — an OpenAI-compatible chat-completions
//! client with tool calling (the roadmap's "scaffolding should get a real
//! model or be deleted": this is the real model).
//!
//! Configuration (F-28 spirit: the agent runs with core privileges, so
//! enabling it is an explicit act):
//!   `agent.api-url` option (default `https://api.openai.com/v1`)
//!   `agent.model`   option (default `gpt-4o-mini`)
//!   key from the `MAGMA_AGENT_API_KEY` env var or the `agent.api-key` option.
//! Without a key the agent falls back to the deterministic BuiltinPlanner.

use serde_json::{json, Value};

const DEFAULT_API_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const MAX_BUFFER_CONTEXT_CHARS: usize = 8_000;
/// Hard bound on one HTTP body so a runaway server cannot exhaust memory.
pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmConfig {
    pub api_url: String,
    pub api_key: String,
    pub model: String,
}

impl LlmConfig {
    /// Read the agent configuration from editor options + the environment.
    pub fn from_options(options: &std::collections::HashMap<String, String>) -> Option<Self> {
        let api_key = std::env::var("MAGMA_AGENT_API_KEY").ok()
            .filter(|k| !k.trim().is_empty())
            .or_else(|| options.get("agent.api-key").cloned().filter(|k| !k.trim().is_empty()))?;
        Some(LlmConfig {
            api_url: options.get("agent.api-url").cloned()
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_API_URL.to_string()),
            api_key,
            model: options.get("agent.model").cloned()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        })
    }

    pub fn is_configured(options: &std::collections::HashMap<String, String>) -> bool {
        Self::from_options(options).is_some()
    }
}

/// One tool invocation the model asked for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// Raw JSON object text of the `arguments` field.
    pub arguments: String,
}

/// The tools the agent can call.  Deliberately narrow: everything routes
/// through the command registry (capability checks and event hooks apply),
/// plus two read-only introspection helpers.
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

/// The system prompt: who the agent is and how it must behave.
pub fn system_prompt() -> String {
    "You are the Magma editor's built-in agent. You fulfill the user's goal \
     by calling tools — editing files, running commands and build tasks, \
     inspecting diagnostics — and then report concisely what you did and \
     what the result was. Prefer run_editor_command with real editor \
     commands (open-file, save-buffer, shell-command, make, run-config, vc, \
     find-files, keyword-context, goto-line, …); use list_editor_commands to \
     discover more. Never fabricate results: if a tool call fails, say so \
     and try a different approach. When the goal is met, answer with a \
     short summary and call no more tools."
        .to_string()
}

/// Context injected as the first user turn so the model starts grounded.
pub fn context_message(ctx: &super::types::AgentContext) -> String {
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

/// One chat completion.  Returns the assistant text and any tool calls.
/// Runs on a background thread; never on the UI thread.
pub fn chat(cfg: &LlmConfig, messages: &[Value]) -> Result<(String, Vec<ToolCall>), String> {
    let body = json!({
        "model": cfg.model,
        "messages": messages,
        "tools": tools_schema(),
    });
    let url = format!("{}/chat/completions", cfg.api_url.trim_end_matches('/'));
    let resp = ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", cfg.api_key))
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(120))
        .send_string(&body.to_string())
        .map_err(|e| format!("agent LLM request failed: {e}"))?;

    let mut raw = String::new();
    use std::io::Read;
    resp.into_reader()
        .take(MAX_RESPONSE_BYTES as u64)
        .read_to_string(&mut raw)
        .map_err(|e| format!("agent LLM read failed: {e}"))?;
    parse_chat_response(&raw)
}

/// Parse a chat-completions response body into (text, tool calls).
/// Exposed for tests — no network involved.
pub fn parse_chat_response(raw: &str) -> Result<(String, Vec<ToolCall>), String> {
    let v: Value = serde_json::from_str(raw)
        .map_err(|e| format!("agent LLM bad JSON: {e}"))?;
    if let Some(err) = v.get("error") {
        let msg = err.get("message").and_then(Value::as_str).unwrap_or("unknown error");
        return Err(format!("agent LLM error: {msg}"));
    }
    let message = v.pointer("/choices/0/message")
        .ok_or_else(|| "agent LLM response has no choices[0].message".to_string())?;
    let text = message.get("content").and_then(Value::as_str).unwrap_or("").to_string();
    let mut calls = Vec::new();
    if let Some(items) = message.get("tool_calls").and_then(Value::as_array) {
        for c in items {
            let function = c.get("function")
                .ok_or_else(|| "tool call missing function".to_string())?;
            calls.push(ToolCall {
                id: c.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
                name: function.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
                arguments: match function.get("arguments") {
                    Some(Value::String(s)) => s.clone(),
                    Some(other) => other.to_string(),
                    None => "{}".to_string(),
                },
            });
        }
    }
    Ok((text, calls))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_requires_a_key_from_env_or_option() {
        // No env var, no option → not configured (the deterministic
        // planner keeps working).
        let empty = std::collections::HashMap::new();
        if std::env::var_os("MAGMA_AGENT_API_KEY").is_none() {
            assert!(!LlmConfig::is_configured(&empty));
        }
        let opts = std::collections::HashMap::from([
            ("agent.api-key".to_string(), "sk-test".to_string()),
        ]);
        let cfg = LlmConfig::from_options(&opts).expect("key from option");
        assert_eq!(cfg.api_url, DEFAULT_API_URL);
        assert_eq!(cfg.model, DEFAULT_MODEL);

        let full = std::collections::HashMap::from([
            ("agent.api-key".to_string(), "sk-test".to_string()),
            ("agent.api-url".to_string(), "http://localhost:8080/v1".to_string()),
            ("agent.model".to_string(), "llama3".to_string()),
        ]);
        let cfg = LlmConfig::from_options(&full).expect("custom config");
        assert_eq!(cfg.api_url, "http://localhost:8080/v1");
        assert_eq!(cfg.model, "llama3");
    }

    #[test]
    fn parses_text_only_response() {
        let raw = r#"{"choices":[{"message":{"role":"assistant","content":"Done."}}]}"#;
        let (text, calls) = parse_chat_response(raw).unwrap();
        assert_eq!(text, "Done.");
        assert!(calls.is_empty());
    }

    #[test]
    fn parses_tool_calls_with_string_arguments() {
        let raw = r#"{
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "run_editor_command",
                            "arguments": "{\"command\":\"open-file\",\"args\":{\"path\":\"src/main.rs\"}}"
                        }
                    }]
                }
            }]
        }"#;
        let (text, calls) = parse_chat_response(raw).unwrap();
        assert!(text.is_empty());
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].name, "run_editor_command");
        let args: Value = serde_json::from_str(&calls[0].arguments).unwrap();
        assert_eq!(args["args"]["path"], "src/main.rs");
    }

    #[test]
    fn api_errors_surface_their_message() {
        let raw = r#"{"error":{"message":"bad key","type":"auth"}}"#;
        let err = parse_chat_response(raw).unwrap_err();
        assert!(err.contains("bad key"));
    }

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
