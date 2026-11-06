//! Agent Client Protocol (ACP) support.
//!
//! ACP is the open standard (JSON-RPC 2.0) for communication between code editors
//! and autonomous AI coding agents (similar to LSP for language servers).
//! Magma implements the client/host side of ACP: managing agent process lifecycles,
//! creating sessions, streaming prompts and updates, and servicing agent requests
//! for filesystem access and command execution.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const ACP_VERSION: &str = "0.1.0";

// ── JSON-RPC 2.0 Base Protocol ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: impl Into<Value>, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: impl Into<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: impl Into<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ── ACP Protocol Messages & Capabilities ─────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ClientCapabilities {
    pub fs: bool,
    pub terminal: bool,
    pub diagnostics: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub client_info: ClientInfo,
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub agent_info: AgentInfo,
    pub capabilities: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNewParams {
    pub root_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNewResult {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionPromptParams {
    pub session_id: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionUpdateNotification {
    pub session_id: String,
    pub update_type: String, // "thought" | "tool_call" | "message" | "status"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

// ── ACP Host / Client Session ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AcpSession {
    pub session_id: String,
    pub root_uri: String,
    pub model: Option<String>,
    pub active: bool,
}

#[derive(Debug, Default)]
pub struct AcpClient {
    next_req_id: u64,
    pub client_info: Option<ClientInfo>,
    pub agent_info: Option<AgentInfo>,
    pub initialized: bool,
}

impl AcpClient {
    pub fn new() -> Self {
        Self {
            next_req_id: 1,
            client_info: Some(ClientInfo {
                name: "Magma".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            }),
            agent_info: None,
            initialized: false,
        }
    }

    /// Build the `initialize` request to send to an ACP agent.
    pub fn build_initialize_request(&mut self) -> JsonRpcRequest {
        let req_id = self.next_req_id;
        self.next_req_id += 1;

        let params = InitializeParams {
            protocol_version: ACP_VERSION.to_string(),
            client_info: self.client_info.clone().unwrap_or(ClientInfo {
                name: "Magma".to_string(),
                version: "0.1.0".to_string(),
            }),
            capabilities: ClientCapabilities {
                fs: true,
                terminal: true,
                diagnostics: true,
            },
        };

        JsonRpcRequest::new(req_id, "initialize", Some(json!(params)))
    }

    /// Process the `initialize` response from the ACP agent.
    pub fn handle_initialize_response(&mut self, res: &JsonRpcResponse) -> Result<InitializeResult, String> {
        if let Some(err) = &res.error {
            return Err(format!("ACP initialize failed: {}", err.message));
        }
        let Some(val) = &res.result else {
            return Err("ACP initialize missing result".to_string());
        };
        let init_result: InitializeResult = serde_json::from_value(val.clone())
            .map_err(|e| format!("Failed to parse ACP InitializeResult: {e}"))?;
        self.agent_info = Some(init_result.agent_info.clone());
        self.initialized = true;
        Ok(init_result)
    }

    /// Build a `session/new` request.
    pub fn build_session_new_request(&mut self, root_uri: &str, model: Option<&str>) -> JsonRpcRequest {
        let req_id = self.next_req_id;
        self.next_req_id += 1;

        let params = SessionNewParams {
            root_uri: root_uri.to_string(),
            model: model.map(|m| m.to_string()),
        };

        JsonRpcRequest::new(req_id, "session/new", Some(json!(params)))
    }

    /// Build a `session/prompt` request.
    pub fn build_session_prompt_request(&mut self, session_id: &str, prompt: &str) -> JsonRpcRequest {
        let req_id = self.next_req_id;
        self.next_req_id += 1;

        let params = SessionPromptParams {
            session_id: session_id.to_string(),
            prompt: prompt.to_string(),
        };

        JsonRpcRequest::new(req_id, "session/prompt", Some(json!(params)))
    }

    /// Build a `session/cancel` notification.
    pub fn build_session_cancel_notification(&self, session_id: &str) -> JsonRpcNotification {
        JsonRpcNotification::new("session/cancel", Some(json!({ "session_id": session_id })))
    }

    /// Handle agent-initiated host calls (fs/readFile, fs/writeFile, terminal/runCommand).
    pub fn handle_host_request<FRead, FWrite, FExec>(
        req: &JsonRpcRequest,
        read_file: FRead,
        write_file: FWrite,
        run_command: FExec,
    ) -> JsonRpcResponse
    where
        FRead: Fn(&str) -> Result<String, String>,
        FWrite: Fn(&str, &str) -> Result<(), String>,
        FExec: Fn(&str) -> Result<(i32, String, String), String>,
    {
        let params = req.params.as_ref().unwrap_or(&Value::Null);
        match req.method.as_str() {
            "fs/readFile" => {
                let path = params.get("path").and_then(Value::as_str).unwrap_or("");
                match read_file(path) {
                    Ok(content) => JsonRpcResponse::success(req.id.clone(), json!({ "content": content })),
                    Err(e) => JsonRpcResponse::error(req.id.clone(), -32001, e),
                }
            }
            "fs/writeFile" => {
                let path = params.get("path").and_then(Value::as_str).unwrap_or("");
                let content = params.get("content").and_then(Value::as_str).unwrap_or("");
                match write_file(path, content) {
                    Ok(()) => JsonRpcResponse::success(req.id.clone(), json!({ "success": true })),
                    Err(e) => JsonRpcResponse::error(req.id.clone(), -32002, e),
                }
            }
            "terminal/runCommand" => {
                let command = params.get("command").and_then(Value::as_str).unwrap_or("");
                match run_command(command) {
                    Ok((exit_code, stdout, stderr)) => JsonRpcResponse::success(
                        req.id.clone(),
                        json!({
                            "exit_code": exit_code,
                            "stdout": stdout,
                            "stderr": stderr,
                        }),
                    ),
                    Err(e) => JsonRpcResponse::error(req.id.clone(), -32003, e),
                }
            }
            other => JsonRpcResponse::error(req.id.clone(), -32601, format!("Method not found: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_flow() {
        let mut client = AcpClient::new();
        let req = client.build_initialize_request();
        assert_eq!(req.method, "initialize");
        assert_eq!(req.jsonrpc, "2.0");

        let response = JsonRpcResponse::success(
            req.id.clone(),
            json!({
                "protocol_version": ACP_VERSION,
                "agent_info": {
                    "name": "AcpAgent",
                    "version": "1.0.0"
                },
                "capabilities": {}
            }),
        );

        let init = client.handle_initialize_response(&response).unwrap();
        assert_eq!(init.agent_info.name, "AcpAgent");
        assert!(client.initialized);
    }

    #[test]
    fn session_requests() {
        let mut client = AcpClient::new();
        let req = client.build_session_new_request("file:///project", Some("claude-3-5"));
        assert_eq!(req.method, "session/new");

        let prompt_req = client.build_session_prompt_request("sess-1", "Fix compiler warning");
        assert_eq!(prompt_req.method, "session/prompt");
    }

    #[test]
    fn host_request_dispatching() {
        let req = JsonRpcRequest::new(1, "fs/readFile", Some(json!({ "path": "src/main.rs" })));
        let res = AcpClient::handle_host_request(
            &req,
            |p| {
                assert_eq!(p, "src/main.rs");
                Ok("fn main() {}".to_string())
            },
            |_, _| Ok(()),
            |_| Ok((0, String::new(), String::new())),
        );
        assert_eq!(res.error, None);
        assert_eq!(res.result.unwrap()["content"], "fn main() {}");
    }
}
