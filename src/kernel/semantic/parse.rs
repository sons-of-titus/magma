//! JSON-RPC response parsers for LSP protocol messages.
//!
//! Each function extracts structured data from a raw JSON-RPC response body.

/// Convert an LSP URI (e.g. `file:///path/to/file`) to a plain path.
pub fn uri_to_path(uri: &str) -> String {
    if let Some(path) = uri.strip_prefix("file://") {
        if path.starts_with('/') {
            path.to_string()
        } else {
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() > 1 {
                format!("/{}", parts[1..].join("/"))
            } else {
                String::new()
            }
        }
    } else {
        uri.to_string()
    }
}

pub fn build_request(id: u64, method: &str, params: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{},"method":"{}","params":{}}}"#, id, method, params)
}

/// Parse a `textDocument/publishDiagnostics` notification.
/// Returns `(uri, diagnostic_strings)` on success.
pub fn parse_diagnostics(body: &str) -> Option<(String, Vec<String>)> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let raw_uri = json.get("params")?.get("uri")?.as_str()?;
    let path = uri_to_path(raw_uri);
    let diags = json.get("params")?.get("diagnostics")?.as_array()?;
    let mut out = Vec::new();
    for d in diags {
        let msg = d.get("message").and_then(|m| m.as_str()).unwrap_or("?");
        let line = d.get("range")
            .and_then(|r| r.get("start")?.get("line")?.as_u64())
            .unwrap_or(0);
        let col = d.get("range")
            .and_then(|r| r.get("start")?.get("character")?.as_u64())
            .unwrap_or(0);
        let sev = d.get("severity").and_then(|s| s.as_u64()).unwrap_or(0);
        let sev_str = match sev {
            1 => "E",
            2 => "W",
            3 => "I",
            4 => "H",
            _ => "?",
        };
        out.push(format!("[{}] line {}:{} {}", sev_str, line + 1, col + 1, msg));
    }
    Some((path, out))
}

/// Parse a `textDocument/hover` response.
/// Returns the hover contents as a formatted string.
pub fn parse_hover(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let result = json.get("result")?;
    if result.is_null() {
        return None;
    }
    let contents = result.get("contents")?;
    let text = if let Some(s) = contents.as_str() {
        s.to_string()
    } else if let Some(obj) = contents.as_object() {
        if let Some(kind) = obj.get("kind").and_then(|k| k.as_str()) {
            if let Some(value) = obj.get("value").and_then(|v| v.as_str()) {
                format!("```{}\n{}\n```", kind, value)
            } else {
                value_or_string(obj.get("value"))?
            }
        } else {
            value_or_string(obj.get("value"))?
        }
    } else if let Some(arr) = contents.as_array() {
        let mut parts = Vec::new();
        for item in arr {
            if let Some(s) = item.as_str() {
                parts.push(s.to_string());
            } else if let Some(obj) = item.as_object()
                && let Some(value) = obj.get("value").and_then(|v| v.as_str()) {
                    parts.push(value.to_string());
                }
        }
        parts.join("\n---\n")
    } else {
        return None;
    };
    Some(text)
}

fn value_or_string(v: Option<&serde_json::Value>) -> Option<String> {
    match v {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(other) => Some(other.to_string()),
        None => None,
    }
}

/// Parse a `textDocument/definition` response.
/// Returns `(uri, start_line, start_char, end_line, end_char)`.
pub fn parse_definition(body: &str) -> Option<(String, usize, usize, usize, usize)> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let result = json.get("result")?;
    if result.is_null() {
        return None;
    }
    // Definition can be a single Location or a LocationLink
    let loc = if result.is_array() {
        result.as_array()?.first()?
    } else {
        result
    };
    let uri = loc.get("uri")?.as_str()?;
    let range = loc.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;
    let sl = start.get("line")?.as_u64()? as usize;
    let sc = start.get("character")?.as_u64()? as usize;
    let el = end.get("line")?.as_u64()? as usize;
    let ec = end.get("character")?.as_u64()? as usize;
    Some((uri_to_path(uri), sl, sc, el, ec))
}

/// Parse a `textDocument/codeAction` response.
/// Returns a JSON array string of available actions (each has title + edit metadata).
pub fn parse_code_actions(body: &str) -> Option<Vec<String>> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let result = json.get("result")?;
    let actions = result.as_array()?;
    let mut out = Vec::new();
    for action in actions {
        if let Some(title) = action.get("title").and_then(|t| t.as_str()) {
            let kind = action.get("kind").and_then(|k| k.as_str()).unwrap_or("quickfix");
            out.push(format!(r#"{{"title":"{}","kind":"{}"}}"#, title, kind));
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Parse a `textDocument/completion` response.
/// Returns a list of item label/detail strings.
pub fn parse_completion_items(body: &str) -> Option<Vec<String>> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let result = json.get("result")?;
    let items = if let Some(arr) = result.as_array() {
        arr
    } else if let Some(obj) = result.as_object() {
        obj.get("items")?.as_array()?
    } else {
        return None;
    };
    let mut out = Vec::new();
    for item in items {
        let label = item.get("label").and_then(|l| l.as_str()).unwrap_or("?");
        let detail = item.get("detail").and_then(|d| d.as_str());
        let insert = item.get("insertText").and_then(|t| t.as_str()).unwrap_or(label);
        out.push(format!(r#"{{"label":"{}","detail":"{}","insertText":"{}"}}"#,
            label, detail.unwrap_or(""), insert));
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Parse a `textDocument/rename` response.
/// Returns the WorkspaceEdit JSON string.
pub fn parse_rename(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let result = json.get("result")?;
    if result.is_null() {
        return None;
    }
    Some(result.to_string())
}

/// Parse a `$/progress` notification.
/// Returns `(token, message, percentage)`.
pub fn parse_progress(body: &str) -> Option<(String, String, String)> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let params = json.get("params")?;
    let token = params.get("token")
        .map(|t| {
            if let Some(s) = t.as_str() { s.to_string() }
            else { t.as_u64().map(|n| n.to_string()).unwrap_or_default() }
        })
        .unwrap_or_default();
    let value = params.get("value")?;
    let message = value.get("message").and_then(|m| m.as_str()).unwrap_or("");
    let percentage = value.get("percentage").and_then(|p| p.as_u64()).map(|p| p.to_string()).unwrap_or_default();
    Some((token, message.to_string(), percentage))
}

/// Extract the `id` field from a JSON-RPC response.
pub fn extract_id(body: &str) -> Option<u64> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    json.get("id")?.as_u64()
}

/// Extract the `method` field from a JSON-RPC notification.
pub fn extract_method(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    json.get("method")?.as_str().map(|s| s.to_string())
}

/// Check if the body has an `id` field (response) vs `method` field (notification).
pub fn is_response(body: &str) -> bool {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        v.get("id").is_some()
    } else {
        false
    }
}

/// Check if the body contains an error field.
pub fn has_error(body: &str) -> bool {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        v.get("error").is_some()
    } else {
        false
    }
}

/// Extract error message from a JSON-RPC error response.
pub fn extract_error(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let error = json.get("error")?;
    let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
    let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    Some(format!("LSP error {}: {}", code, msg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_to_path() {
        assert_eq!(uri_to_path("file:///home/user/file.rs"), "/home/user/file.rs");
        assert_eq!(uri_to_path("/raw/path"), "/raw/path");
    }

    #[test]
    fn test_parse_diagnostics() {
        let body = r#"{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":"file:///test.rs","diagnostics":[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}},"severity":1,"message":"test error"}]}}"#;
        let (path, diags) = parse_diagnostics(body).unwrap();
        assert_eq!(path, "/test.rs");
        assert!(diags[0].contains("[E]"));
        assert!(diags[0].contains("test error"));
    }

    #[test]
    fn test_parse_hover_markdown() {
        let body = r##"{"jsonrpc":"2.0","id":1,"result":{"contents":{"kind":"markdown","value":"# Title\n\nBody"}}}"##;
        let text = parse_hover(body).unwrap();
        assert!(text.contains("Title"));
        assert!(text.contains("Body"));
    }

    #[test]
    fn test_parse_hover_plain() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":{"contents":"plain text"}}"#;
        assert_eq!(parse_hover(body).unwrap(), "plain text");
    }

    #[test]
    fn test_parse_definition() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":[{"uri":"file:///src/lib.rs","range":{"start":{"line":10,"character":5},"end":{"line":10,"character":20}}}]}"#;
        let (uri, sl, sc, el, ec) = parse_definition(body).unwrap();
        assert_eq!(uri, "/src/lib.rs");
        assert_eq!(sl, 10);
        assert_eq!(sc, 5);
        assert_eq!(el, 10);
        assert_eq!(ec, 20);
    }

    #[test]
    fn test_parse_code_actions() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":[{"title":"Fix it","kind":"quickfix"},{"title":"Refactor","kind":"refactor"}]}"#;
        let actions = parse_code_actions(body).unwrap();
        assert_eq!(actions.len(), 2);
        assert!(actions[0].contains("Fix it"));
    }

    #[test]
    fn test_parse_completion_items() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":[{"label":"println","detail":"macro"},{"label":"format","detail":"macro"}]}"#;
        let items = parse_completion_items(body).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_parse_rename() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":{"changes":{"/file.rs":[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":5}},"newText":"newname"}]}}}"#;
        let edit = parse_rename(body).unwrap();
        assert!(edit.contains("changes"));
    }

    #[test]
    fn test_parse_progress() {
        let body = r#"{"jsonrpc":"2.0","method":"$/progress","params":{"token":"build","value":{"kind":"begin","message":"Building...","percentage":0}}}"#;
        let (token, msg, pct) = parse_progress(body).unwrap();
        assert_eq!(token, "build");
        assert_eq!(msg, "Building...");
    }

    #[test]
    fn test_extract_id() {
        assert_eq!(extract_id(r#"{"id":42,"result":null}"#), Some(42));
        assert_eq!(extract_id(r#"{"method":"test"}"#), None);
    }

    #[test]
    fn test_extract_method() {
        assert_eq!(extract_method(r#"{"method":"test"}"#), Some("test".to_string()));
        assert_eq!(extract_method(r#"{"id":1,"result":null}"#), None);
    }

    #[test]
    fn test_is_response() {
        assert!(is_response(r#"{"id":1,"result":null}"#));
        assert!(!is_response(r#"{"method":"test"}"#));
    }

    #[test]
    fn test_extract_error() {
        let body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"method not found"}}"#;
        let err = extract_error(body).unwrap();
        assert!(err.contains("method not found"));
    }
}
