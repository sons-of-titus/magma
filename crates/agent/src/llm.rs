//! LLM backend for the agent loop.

use serde_json::{Value, json};

const DEFAULT_API_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const MAX_BUFFER_CONTEXT_CHARS: usize = 8_000;

/// A known LLM provider the agent can talk to.  All of them expose an
/// OpenAI-compatible `/chat/completions` endpoint, so the same client works
/// for every entry — only the base URL and default model differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    pub name: &'static str,
    pub api_url: &'static str,
    pub default_model: &'static str,
    pub models: &'static [&'static str],
}

/// Known providers, in priority order: American AI first, then the Chinese
/// providers (DeepSeek, Kimi, Zai).
pub const PROVIDERS: &[Provider] = &[
    Provider {
        name: "openai",
        api_url: "https://api.openai.com/v1",
        default_model: "gpt-4o-mini",
        models: &["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "o1", "o1-mini"],
    },
    Provider {
        name: "deepseek",
        api_url: "https://api.deepseek.com/v1",
        default_model: "deepseek-chat",
        models: &["deepseek-chat", "deepseek-reasoner"],
    },
    Provider {
        name: "kimi",
        api_url: "https://api.moonshot.cn/v1",
        default_model: "moonshot-v1-8k",
        models: &["moonshot-v1-8k", "moonshot-v1-32k", "moonshot-v1-128k"],
    },
    Provider {
        name: "zai",
        api_url: "https://open.bigmodel.cn/api/paas/v4",
        default_model: "glm-4-flash",
        models: &["glm-4", "glm-4-flash", "glm-4-plus", "glm-4-air"],
    },
];

pub fn known_providers() -> &'static [Provider] {
    PROVIDERS
}
pub fn providers() -> &'static [Provider] {
    PROVIDERS
}
pub fn list_providers() -> &'static [Provider] {
    PROVIDERS
}
pub fn available_providers() -> &'static [Provider] {
    PROVIDERS
}

pub fn models_for_provider(name: &str) -> &'static [&'static str] {
    provider_by_name(name).map(|p| p.models).unwrap_or(&[])
}

pub fn known_models(name: &str) -> &'static [&'static str] {
    models_for_provider(name)
}
pub fn models(name: &str) -> &'static [&'static str] {
    models_for_provider(name)
}

pub fn provider_by_name(name: &str) -> Option<&'static Provider> {
    let n = name.trim().to_lowercase();
    let idx = match n.as_str() {
        "openai" | "open-ai" | "open ai" | "gpt" | "chatgpt" | "american" | "american ai"
        | "us" | "usa" => Some(0),
        "deepseek" | "deep-seek" | "deep seek" => Some(1),
        "kimi" | "moonshot" | "moonshot ai" => Some(2),
        "zai" | "zhipu" | "glm" | "bigmodel" | "chatglm" => Some(3),
        _ => None,
    };
    if let Some(i) = idx {
        return Some(&PROVIDERS[i]);
    }
    PROVIDERS.iter().find(|p| p.name == n)
}

/// Names of the known providers, in priority order.
pub const PROVIDER_NAMES: &[&str] = &["openai", "deepseek", "kimi", "zai"];
pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmConfig {
    pub api_url: String,
    pub api_key: String,
    pub model: String,
}

impl LlmConfig {
    pub fn from_options(options: &std::collections::HashMap<String, String>) -> Option<Self> {
        // Resolve the provider first so we can pick its dedicated token
        // (`agent.api-key.<provider>`) when one is stored, falling back to the
        // shared `agent.api-key` and finally the environment.
        let provider_name = std::env::var("MAGMA_AGENT_PROVIDER")
            .ok()
            .filter(|p| !p.trim().is_empty())
            .or_else(|| {
                options
                    .get("agent.provider")
                    .cloned()
                    .filter(|p| !p.trim().is_empty())
            });
        let provider = provider_name.as_deref().and_then(provider_by_name);

        let mut api_key = None;
        if let Some(p) = provider {
            let key_name = format!("agent.api-key.{}", p.name);
            api_key = options
                .get(&key_name)
                .cloned()
                .filter(|k| !k.trim().is_empty());
        }
        if api_key.is_none() {
            api_key = options
                .get("agent.api-key")
                .cloned()
                .filter(|k| !k.trim().is_empty());
        }
        if api_key.is_none() {
            api_key = std::env::var("MAGMA_AGENT_API_KEY")
                .ok()
                .filter(|k| !k.trim().is_empty());
        }
        let api_key = api_key?;
        let api_url = options
            .get("agent.api-url")
            .cloned()
            .filter(|u| !u.trim().is_empty())
            .or_else(|| provider.map(|p| p.api_url.to_string()))
            .unwrap_or_else(|| DEFAULT_API_URL.to_string());
        let model = options
            .get("agent.model")
            .cloned()
            .filter(|m| !m.trim().is_empty())
            .or_else(|| provider.map(|p| p.default_model.to_string()))
            .unwrap_or_else(|| {
                provider
                    .map(|p| p.default_model.to_string())
                    .unwrap_or_else(|| DEFAULT_MODEL.to_string())
            });
        Some(LlmConfig {
            api_url,
            api_key,
            model,
        })
    }

    pub fn is_configured(options: &std::collections::HashMap<String, String>) -> bool {
        Self::from_options(options).is_some()
    }
}

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

pub fn parse_chat_response(raw: &str) -> Result<(String, Vec<ToolCall>), String> {
    let v: Value = serde_json::from_str(raw).map_err(|e| format!("agent LLM bad JSON: {e}"))?;
    if let Some(err) = v.get("error") {
        let msg = err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return Err(format!("agent LLM error: {msg}"));
    }
    let message = v
        .pointer("/choices/0/message")
        .ok_or_else(|| "agent LLM response has no choices[0].message".to_string())?;
    let text = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut calls = Vec::new();
    if let Some(items) = message.get("tool_calls").and_then(Value::as_array) {
        for c in items {
            let function = c
                .get("function")
                .ok_or_else(|| "tool call missing function".to_string())?;
            calls.push(ToolCall {
                id: c
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                name: function
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
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
        let empty = std::collections::HashMap::new();
        if std::env::var_os("MAGMA_AGENT_API_KEY").is_none() {
            assert!(!LlmConfig::is_configured(&empty));
        }
        let opts =
            std::collections::HashMap::from([("agent.api-key".to_string(), "sk-test".to_string())]);
        let cfg = LlmConfig::from_options(&opts).expect("key from option");
        assert_eq!(cfg.api_url, DEFAULT_API_URL);
        assert_eq!(cfg.model, DEFAULT_MODEL);

        let full = std::collections::HashMap::from([
            ("agent.api-key".to_string(), "sk-test".to_string()),
            (
                "agent.api-url".to_string(),
                "http://localhost:8080/v1".to_string(),
            ),
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
    fn provider_resolution_picks_known_provider_defaults() {
        let opts = std::collections::HashMap::from([
            ("agent.api-key".to_string(), "sk-test".to_string()),
            ("agent.provider".to_string(), "deepseek".to_string()),
        ]);
        let cfg = LlmConfig::from_options(&opts).expect("configured");
        assert_eq!(cfg.api_url, "https://api.deepseek.com/v1");
        assert_eq!(cfg.model, "deepseek-chat");

        let opts = std::collections::HashMap::from([
            ("agent.api-key".to_string(), "sk-test".to_string()),
            ("agent.provider".to_string(), "kimi".to_string()),
        ]);
        let cfg = LlmConfig::from_options(&opts).expect("configured");
        assert_eq!(cfg.api_url, "https://api.moonshot.cn/v1");

        let opts = std::collections::HashMap::from([
            ("agent.api-key".to_string(), "sk-test".to_string()),
            ("agent.provider".to_string(), "zai".to_string()),
        ]);
        let cfg = LlmConfig::from_options(&opts).expect("configured");
        assert_eq!(cfg.api_url, "https://open.bigmodel.cn/api/paas/v4");

        // explicit url/model override provider defaults
        let opts = std::collections::HashMap::from([
            ("agent.api-key".to_string(), "sk-test".to_string()),
            ("agent.provider".to_string(), "deepseek".to_string()),
            (
                "agent.api-url".to_string(),
                "http://localhost:9999/v1".to_string(),
            ),
            ("agent.model".to_string(), "custom-model".to_string()),
        ]);
        let cfg = LlmConfig::from_options(&opts).expect("configured");
        assert_eq!(cfg.api_url, "http://localhost:9999/v1");
        assert_eq!(cfg.model, "custom-model");
    }

    #[test]
    fn per_provider_token_is_preferred() {
        let opts = std::collections::HashMap::from([
            ("agent.provider".to_string(), "kimi".to_string()),
            ("agent.api-key".to_string(), "shared".to_string()),
            ("agent.api-key.kimi".to_string(), "kimi-secret".to_string()),
        ]);
        let cfg = LlmConfig::from_options(&opts).expect("configured");
        assert_eq!(cfg.api_key, "kimi-secret");
    }

    #[test]
    fn known_providers_has_four_in_priority_order() {
        let ps = known_providers();
        assert_eq!(ps.len(), 4);
        assert_eq!(ps[0].name, "openai");
        assert_eq!(ps[1].name, "deepseek");
        assert_eq!(ps[2].name, "kimi");
        assert_eq!(ps[3].name, "zai");
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
