//! DAP (Debug Adapter Protocol) client — spawns a debug adapter subprocess,
//! speaks the DAP wire protocol, and dispatches responses back through the
//! background event channel.
//!
//! The protocol uses the same `Content-Length: N\r\n\r\n{body}` framing as LSP/JSON-RPC.

use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;
use serde_json::Value;

use crate::kernel::runtime::BackgroundEvent;
use super::types::SessionId;

/// A handle for writing DAP requests to a running debug adapter.
pub struct DapClient {
    pub stdin: Arc<AsyncMutex<tokio::process::ChildStdin>>,
    pub bg: crate::kernel::runtime::BackgroundHandle,
    pub session_id: SessionId,
    /// Shared sequence counter — also stored in the static registry so sync
    /// Janet C functions can allocate new sequence numbers without holding the
    /// async mutex.
    pub next_seq: Arc<std::sync::Mutex<i64>>,
}

impl DapClient {
    /// Spawn the debug adapter binary, perform the DAP `initialize` handshake,
    /// and start the background reader task.
    pub async fn start(
        adapter: &str,
        session_id: SessionId,
        bg: crate::kernel::runtime::BackgroundHandle,
    ) -> Result<Self, String> {
        let mut child = Command::new(adapter)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn debug adapter '{}': {e}", adapter))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| "Failed to open adapter stdin".to_string())?;
        let stdout = child.stdout.take()
            .ok_or_else(|| "Failed to open adapter stdout".to_string())?;

        let stdin = Arc::new(AsyncMutex::new(stdin));
        let next_seq = Arc::new(std::sync::Mutex::new(1i64));

        {
            let bg_reader = bg.clone();
            tokio::spawn(async move {
                read_loop(stdout, bg_reader, session_id).await;
            });
        }

        let client = DapClient { stdin, bg: bg.clone(), session_id, next_seq };

        let seq = client.alloc_seq();
        let init_args = r#"{"clientID":"magma","adapterID":"magma","linesStartAt1":true,"columnsStartAt1":true,"pathFormat":"path"}"#;
        let msg = dap_request(seq, "initialize", init_args);
        client.write_raw(&msg).await?;

        Ok(client)
    }

    fn alloc_seq(&self) -> i64 {
        let mut guard = self.next_seq.lock().unwrap_or_else(|e| e.into_inner());
        let s = *guard;
        *guard += 1;
        s
    }

    /// Send a DAP request with the given `command` and `arguments` JSON string.
    pub async fn send(&self, command: &str, arguments: &str) -> Result<(), String> {
        let seq = self.alloc_seq();
        let msg = dap_request(seq, command, arguments);
        self.write_raw(&msg).await
    }

    /// Write a raw DAP message (JSON body) to the adapter over the wire.
    pub async fn write_raw(&self, body: &str) -> Result<(), String> {
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(header.as_bytes()).await
            .map_err(|e| format!("DAP write: {e}"))?;
        stdin.write_all(body.as_bytes()).await
            .map_err(|e| format!("DAP write: {e}"))?;
        stdin.flush().await
            .map_err(|e| format!("DAP flush: {e}"))
    }
}

/// Build a DAP request JSON body.
pub fn dap_request(seq: i64, command: &str, arguments: &str) -> String {
    format!(r#"{{"seq":{seq},"type":"request","command":"{command}","arguments":{arguments}}}"#)
}

/// Background reader: parse DAP messages from adapter stdout and emit BackgroundEvents.
async fn read_loop(
    stdout: tokio::process::ChildStdout,
    bg: crate::kernel::runtime::BackgroundHandle,
    session_id: SessionId,
) {
    let mut reader = BufReader::new(stdout);
    let mut header_lines: Vec<String> = Vec::new();

    loop {
        header_lines.clear();
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) | Err(_) => {
                    bg.send_event(BackgroundEvent::DapTerminated { session_id });
                    return;
                }
                Ok(_) => {
                    let trimmed = line.trim().to_string();
                    if trimmed.is_empty() { break; }
                    header_lines.push(trimmed);
                }
            }
        }

        let content_length: usize = header_lines.iter().find_map(|h| {
            h.strip_prefix("Content-Length: ")
                .and_then(|s| s.trim().parse().ok())
        }).unwrap_or(0);

        if content_length == 0 { continue; }

        let mut body = vec![0u8; content_length];
        if reader.read_exact(&mut body).await.is_err() {
            bg.send_event(BackgroundEvent::DapTerminated { session_id });
            return;
        }

        dispatch_dap_message(&String::from_utf8_lossy(&body), &bg, session_id);
    }
}

/// Parse a DAP message body and emit the appropriate `BackgroundEvent`.
fn dispatch_dap_message(
    body: &str,
    bg: &crate::kernel::runtime::BackgroundHandle,
    session_id: SessionId,
) {
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return,
    };

    let msg_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match msg_type {
        "event" => {
            let event_name = v.get("event").and_then(|e| e.as_str()).unwrap_or("");
            match event_name {
                "stopped" => {
                    let reason = v["body"]["reason"].as_str()
                        .unwrap_or("unknown").to_string();
                    let thread_id = v["body"]["threadId"].as_u64().unwrap_or(0);
                    bg.send_event(BackgroundEvent::DapStopped { session_id, reason, thread_id });
                }
                "output" => {
                    let category = v["body"]["category"].as_str()
                        .unwrap_or("console").to_string();
                    let output = v["body"]["output"].as_str()
                        .unwrap_or("").to_string();
                    bg.send_event(BackgroundEvent::DapOutput { session_id, category, output });
                }
                "terminated" | "exited" => {
                    bg.send_event(BackgroundEvent::DapTerminated { session_id });
                }
                _ => {
                    bg.send_event(BackgroundEvent::DapResponse {
                        session_id,
                        event_type: event_name.to_string(),
                        data: body.to_string(),
                    });
                }
            }
        }
        "response" => {
            let command = v.get("command").and_then(|c| c.as_str()).unwrap_or("");
            if command == "evaluate" && v["success"].as_bool().unwrap_or(false) {
                let result = v["body"]["result"].as_str().unwrap_or("").to_string();
                bg.send_event(BackgroundEvent::DapResponse {
                    session_id,
                    event_type: "evaluate".to_string(),
                    data: result,
                });
            }
        }
        _ => {}
    }
}
