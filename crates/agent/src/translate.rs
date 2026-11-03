//! Request translation and reply parsing per wire protocol.
//!
//! Two protocols cover the built-in providers: OpenAI-compatible
//! `/chat/completions` (with per-provider reasoning parameter styles) and
//! Anthropic's `/v1/messages`.  The agent loop's internal transcript is
//! always OpenAI-shaped; the Anthropic translator converts it on the way out.

use serde_json::{Value, json};

use crate::auth::Credential;
use crate::catalog::{ReasoningSupport, ReasoningLevel};
use crate::llm::ToolCall;
use crate::registry::{AuthStyle, ChatTurn, HttpReq, ProviderMeta};

/// How a selected reasoning level lands in the request body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningStyle {
    /// `reasoning_effort: "<param>"` (OpenAI, Copilot).
    Effort,
    /// `reasoning: { effort: "<param>" }` (OpenRouter).
    OpenRouter,
    /// `thinking: { type: "<param>" }` (Z.ai GLM).
    Toggle,
    /// Anthropic extended thinking with a token budget.
    ThinkingBudget,
    None,
}

fn auth_header(style: AuthStyle, cred: &Credential) -> (String, String) {
    match style {
        AuthStyle::Bearer => ("Authorization".into(), format!("Bearer {}", cred.secret())),
        AuthStyle::XApiKey => ("x-api-key".into(), cred.secret().into()),
    }
}

fn common_headers(style: AuthStyle, cred: &Credential) -> Vec<(String, String)> {
    let mut headers = vec![auth_header(style, cred)];
    headers.push(("User-Agent".into(), "magma-agent/0.1".into()));
    headers
}

/// Apply a reasoning level to an OpenAI-compatible body per provider style.
fn apply_reasoning(body: &mut Value, style: ReasoningStyle, reasoning: Option<&ReasoningLevel>) {
    let Some(level) = reasoning else { return };
    match style {
        ReasoningStyle::Effort => {
            body["reasoning_effort"] = json!(level.param);
        }
        ReasoningStyle::OpenRouter => {
            body["reasoning"] = json!({ "effort": level.param });
        }
        ReasoningStyle::Toggle => {
            body["thinking"] = json!({ "type": level.param });
        }
        ReasoningStyle::ThinkingBudget | ReasoningStyle::None => {}
    }
}

/// OpenAI-compatible chat request (OpenAI, OpenRouter, Copilot, DeepSeek,
/// Kimi, Z.ai).
pub fn openai_compatible(
    meta: &ProviderMeta,
    style: ReasoningStyle,
    turn: &ChatTurn,
    cred: &Credential,
) -> HttpReq {
    let mut body = json!({
        "model": turn.model,
        "messages": turn.messages,
    });
    if turn.tools.as_array().is_some_and(|t| !t.is_empty()) {
        body["tools"] = turn.tools.clone();
    }
    apply_reasoning(&mut body, style, turn.reasoning);
    let url = format!(
        "{}/{}",
        meta.chat_base.trim_end_matches('/'),
        meta.chat_path.trim_start_matches('/')
    );
    HttpReq {
        url,
        headers: common_headers(meta.auth_style, cred),
        body: body.to_string(),
    }
}

/// Anthropic Messages request.  Converts the OpenAI-shaped transcript and
/// wires extended thinking budgets (max_tokens must exceed the budget).
pub fn anthropic(turn: &ChatTurn, cred: &Credential) -> HttpReq {
    let mut system = String::new();
    let mut messages: Vec<Value> = Vec::new();
    for m in turn.messages {
        let role = m.get("role").and_then(Value::as_str).unwrap_or("user");
        match role {
            "system" => {
                if let Some(text) = m.get("content").and_then(Value::as_str) {
                    if !system.is_empty() {
                        system.push_str("\n\n");
                    }
                    system.push_str(text);
                }
            }
            "assistant" => {
                let mut blocks: Vec<Value> = Vec::new();
                if let Some(text) = m.get("content").and_then(Value::as_str)
                    && !text.is_empty()
                {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                if let Some(calls) = m.get("tool_calls").and_then(Value::as_array) {
                    for call in calls {
                        let input = call
                            .pointer("/function/arguments")
                            .and_then(Value::as_str)
                            .and_then(|a| serde_json::from_str::<Value>(a).ok())
                            .unwrap_or(json!({}));
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": call.get("id").cloned().unwrap_or(json!("tool")),
                            "name": call.pointer("/function/name").cloned().unwrap_or(json!("?")),
                            "input": input,
                        }));
                    }
                }
                if !blocks.is_empty() {
                    messages.push(json!({ "role": "assistant", "content": blocks }));
                }
            }
            "tool" => {
                let result = json!({
                    "type": "tool_result",
                    "tool_use_id": m.get("tool_call_id").cloned().unwrap_or(json!("tool")),
                    "content": m.get("content").cloned().unwrap_or(json!("")),
                });
                match messages.last_mut() {
                    Some(last) if last["role"] == "user" && last["content"].is_array() => {
                        last["content"].as_array_mut().unwrap().push(result);
                    }
                    _ => messages.push(json!({ "role": "user", "content": [result] })),
                }
            }
            _ => {
                let content = m.get("content").cloned().unwrap_or(json!(""));
                messages.push(json!({ "role": "user", "content": content }));
            }
        }
    }

    let mut body = json!({
        "model": turn.model,
        "max_tokens": 8_192,
        "messages": messages,
    });
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    if turn.tools.as_array().is_some_and(|t| !t.is_empty()) {
        let tools: Vec<Value> = turn
            .tools
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                let f = &t["function"];
                json!({
                    "name": f["name"],
                    "description": f["description"],
                    "input_schema": f["parameters"],
                })
            })
            .collect();
        body["tools"] = json!(tools);
    }
    if let Some(level) = turn.reasoning {
        if let Ok(budget) = level.param.parse::<u64>() {
            body["thinking"] = json!({ "type": "enabled", "budget_tokens": budget });
            body["max_tokens"] = json!(budget + 4_096);
        }
    } else {
        body["max_tokens"] = json!(16_384);
    }

    HttpReq {
        url: "https://api.anthropic.com/v1/messages".into(),
        headers: {
            let mut h = common_headers(AuthStyle::XApiKey, cred);
            h.push(("anthropic-version".into(), "2023-06-01".into()));
            h
        },
        body: body.to_string(),
    }
}

// ── Reply parsing ────────────────────────────────────────────────────────────

/// OpenAI-compatible reply: `choices[0].message` with optional tool_calls.
pub fn parse_openai_reply(raw: &str) -> Result<(String, Vec<ToolCall>), String> {
    let v: Value =
        serde_json::from_str(raw).map_err(|e| format!("agent LLM bad JSON: {e}"))?;
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

/// Anthropic reply: `content` blocks of type text / tool_use.
pub fn parse_anthropic_reply(raw: &str) -> Result<(String, Vec<ToolCall>), String> {
    let v: Value =
        serde_json::from_str(raw).map_err(|e| format!("agent LLM bad JSON: {e}"))?;
    if let Some(err) = v.get("error") {
        let msg = err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return Err(format!("agent LLM error: {msg}"));
    }
    let blocks = v
        .get("content")
        .and_then(Value::as_array)
        .ok_or("anthropic reply has no content blocks")?;
    let mut text = String::new();
    let mut calls = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(t) = block.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(t);
                }
            }
            Some("tool_use") => {
                calls.push(ToolCall {
                    id: block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    name: block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    arguments: block
                        .get("input")
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| "{}".into()),
                });
            }
            _ => {}
        }
    }
    Ok((text, calls))
}

/// Level lookup helper for providers wiring `ReasoningSupport` into a
/// request: returns the level with `id`, if the model exposes it.
pub fn level_of(support: &ReasoningSupport, id: &str) -> Option<ReasoningLevel> {
    support
        .levels()
        .iter()
        .find(|l| l.id == id)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ProviderMeta;

    fn openai_meta() -> ProviderMeta {
        ProviderMeta {
            id: "openai",
            display_name: "OpenAI",
            description: "",
            glyph: "",
            auth_methods: &[],
            auth_style: AuthStyle::Bearer,
            chat_base: "https://api.openai.com/v1",
            chat_path: "/chat/completions",
        }
    }

    fn bearer_credential() -> Credential {
        Credential::ApiKey {
            key: "sk-test".into(),
        }
    }

    #[test]
    fn openai_body_carries_model_messages_and_effort() {
        let meta = openai_meta();
        let messages = vec![json!({"role":"user","content":"hi"})];
        let reasoning = ReasoningLevel {
            id: "high".into(),
            label: "High".into(),
            param: "high".into(),
        };
        let turn = ChatTurn {
            model: "gpt-5",
            messages: &messages,
            reasoning: Some(&reasoning),
            tools: &json!([]),
        };
        let req = openai_compatible(&meta, ReasoningStyle::Effort, &turn, &bearer_credential());
        assert_eq!(req.url, "https://api.openai.com/v1/chat/completions");
        assert!(
            req.headers
                .iter()
                .any(|(k, v)| k == "Authorization" && v == "Bearer sk-test")
        );
        let body: Value = serde_json::from_str(&req.body).unwrap();
        assert_eq!(body["model"], "gpt-5");
        assert_eq!(body["reasoning_effort"], "high");
        // No tools → no tools key.
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn openrouter_and_toggle_styles_shape_reasoning_differently() {
        let meta = openai_meta();
        let messages = vec![json!({"role":"user","content":"hi"})];
        let reasoning = ReasoningLevel {
            id: "high".into(),
            label: "High".into(),
            param: "high".into(),
        };
        let turn = ChatTurn {
            model: "m",
            messages: &messages,
            reasoning: Some(&reasoning),
            tools: &json!([]),
        };
        let req = openai_compatible(&meta, ReasoningStyle::OpenRouter, &turn, &bearer_credential());
        let body: Value = serde_json::from_str(&req.body).unwrap();
        assert_eq!(body["reasoning"]["effort"], "high");
        assert!(body.get("reasoning_effort").is_none());

        let toggle = ReasoningLevel {
            id: "on".into(),
            label: "Enabled".into(),
            param: "enabled".into(),
        };
        let turn = ChatTurn {
            model: "glm-4.6",
            messages: &messages,
            reasoning: Some(&toggle),
            tools: &json!([]),
        };
        let req = openai_compatible(&meta, ReasoningStyle::Toggle, &turn, &bearer_credential());
        let body: Value = serde_json::from_str(&req.body).unwrap();
        assert_eq!(body["thinking"]["type"], "enabled");
    }

    #[test]
    fn anthropic_request_converts_the_transcript() {
        let messages = vec![
            json!({"role":"system","content":"be brief"}),
            json!({"role":"user","content":"list files"}),
            json!({"role":"assistant","content":"","tool_calls":[
                {"id":"t1","type":"function","function":{"name":"ls","arguments":"{\"path\":\".\"}"}}
            ]}),
            json!({"role":"tool","tool_call_id":"t1","content":"main.rs"}),
        ];
        let reasoning = ReasoningLevel {
            id: "medium".into(),
            label: "Medium".into(),
            param: "8192".into(),
        };
        let turn = ChatTurn {
            model: "claude-sonnet-4-5",
            messages: &messages,
            reasoning: Some(&reasoning),
            tools: &json!([{"type":"function","function":{"name":"ls","description":"list","parameters":{"type":"object"}}}]),
        };
        let req = anthropic(&turn, &bearer_credential());
        assert_eq!(req.url, "https://api.anthropic.com/v1/messages");
        assert!(
            req.headers
                .iter()
                .any(|(k, v)| k == "x-api-key" && v == "sk-test")
        );
        assert!(
            req.headers
                .iter()
                .any(|(k, v)| k == "anthropic-version" && v == "2023-06-01")
        );
        let body: Value = serde_json::from_str(&req.body).unwrap();
        assert_eq!(body["system"], "be brief");
        assert_eq!(body["thinking"]["budget_tokens"], 8192);
        assert!(body["max_tokens"].as_u64().unwrap() > 8192, "budget needs headroom");
        assert_eq!(body["tools"][0]["name"], "ls");
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[1]["content"][0]["type"], "tool_use");
        assert_eq!(msgs[1]["content"][0]["input"]["path"], ".");
        assert_eq!(msgs[2]["content"][0]["type"], "tool_result");
        assert_eq!(msgs[2]["content"][0]["tool_use_id"], "t1");
    }

    #[test]
    fn anthropic_without_reasoning_uses_plain_budget() {
        let messages = vec![json!({"role":"user","content":"hi"})];
        let turn = ChatTurn {
            model: "claude-3-5-haiku",
            messages: &messages,
            reasoning: None,
            tools: &json!([]),
        };
        let req = anthropic(&turn, &bearer_credential());
        let body: Value = serde_json::from_str(&req.body).unwrap();
        assert!(body.get("thinking").is_none());
        assert_eq!(body["max_tokens"], 16_384);
    }

    #[test]
    fn openai_reply_parses_text_and_tool_calls() {
        let raw = r#"{"choices":[{"message":{"role":"assistant","content":null,"tool_calls":[
            {"id":"call_1","type":"function","function":{"name":"run_editor_command","arguments":"{\"command\":\"open-file\"}"}}
        ]}}]}"#;
        let (text, calls) = parse_openai_reply(raw).unwrap();
        assert!(text.is_empty());
        assert_eq!(calls[0].name, "run_editor_command");
        assert_eq!(calls[0].arguments, r#"{"command":"open-file"}"#);
    }

    #[test]
    fn anthropic_reply_joins_text_and_extracts_tool_use() {
        let raw = r#"{"content":[
            {"type":"text","text":"Opening"},
            {"type":"tool_use","id":"tu_1","name":"read_active_buffer","input":{"x":1}}
        ]}"#;
        let (text, calls) = parse_anthropic_reply(raw).unwrap();
        assert_eq!(text, "Opening");
        assert_eq!(calls[0].id, "tu_1");
        assert_eq!(calls[0].arguments, r#"{"x":1}"#);
    }

    #[test]
    fn api_errors_surface_their_message() {
        assert!(parse_openai_reply(r#"{"error":{"message":"bad key"}}"#)
            .unwrap_err()
            .contains("bad key"));
        assert!(parse_anthropic_reply(r#"{"type":"error","error":{"message":"nope"}}"#)
            .unwrap_err()
            .contains("nope"));
    }
}
