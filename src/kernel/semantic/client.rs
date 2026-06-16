//! LSP client — spawns a language server subprocess, speaks JSON-RPC,
//! and dispatches responses back through the background event channel.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;

use crate::kernel::runtime::BackgroundEvent;
use super::parse;

pub type PendingMap = Arc<Mutex<HashMap<u64, String>>>;

static PENDING_REQUESTS: LazyLock<PendingMap> = LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

pub fn pending_map() -> PendingMap {
    PENDING_REQUESTS.clone()
}

/// A running language server connection.
#[allow(dead_code)]
pub struct LspServer {
    pub stdin: Arc<AsyncMutex<tokio::process::ChildStdin>>,
    pub bg: crate::kernel::runtime::BackgroundHandle,
    pub language: String,
    pub next_id: u64,
    pub pending: PendingMap,
}

impl LspServer {
    /// Start a language server process and perform the initialize handshake.
    pub async fn start(
        command: &str,
        args: &[String],
        language: &str,
        bg: crate::kernel::runtime::BackgroundHandle,
    ) -> Result<Self, String> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn LSP server '{}': {}", command, e))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| "Failed to open LSP server stdin".to_string())?;
        let stdout = child.stdout.take()
            .ok_or_else(|| "Failed to open LSP server stdout".to_string())?;

        let stdin = Arc::new(AsyncMutex::new(stdin));
        let pending = pending_map();

        {
            let bg = bg.clone();
            let language = language.to_string();
            let pending = pending.clone();
            tokio::spawn(async move {
                read_loop(stdout, bg, &language, pending).await;
            })
        };

        let mut server = LspServer {
            stdin,
            bg: bg.clone(),
            language: language.to_string(),
            next_id: 1,
            pending,
        };

        let init_result = server.initialize().await?;
        bg.send_event(BackgroundEvent::Custom(
            "lsp-initialized".to_string(),
            format!("{}: {}", language, init_result),
        ));

        server.send_notification("initialized", "{}").await?;

        Ok(server)
    }

    /// Send the initialize request and return the server's capabilities.
    async fn initialize(&mut self) -> Result<String, String> {
        let params = r#"{"processId":null,"capabilities":{},"rootUri":null}"#;
        let id = self.next_id;
        self.next_id += 1;
        let request = parse::build_request(id, "initialize", params);
        self.write(&request).await?;
        Ok(format!("initialized (id={})", id))
    }

    /// Notify the server that a buffer was opened.
    pub async fn did_open(&self, uri: &str, language_id: &str, text: &str) -> Result<(), String> {
        let text = text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        let params = format!(
            r#"{{"textDocument":{{"uri":"{}","languageId":"{}","version":1,"text":"{}"}}}}"#,
            uri, language_id, text
        );
        self.send_notification("textDocument/didOpen", &params).await
    }

    /// Notify the server that a buffer was changed.
    pub async fn did_change(&self, uri: &str, text: &str) -> Result<(), String> {
        let text = text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        let params = format!(
            r#"{{"textDocument":{{"uri":"{}","version":2}},"contentChanges":[{{"text":"{}"}}]}}"#,
            uri, text
        );
        self.send_notification("textDocument/didChange", &params).await
    }

    /// Notify the server that a buffer was closed.
    pub async fn did_close(&self, uri: &str) -> Result<(), String> {
        let params = format!(r#"{{"textDocument":{{"uri":"{}"}}}}"#, uri);
        self.send_notification("textDocument/didClose", &params).await
    }

    /// Send a generic JSON-RPC request and track it for response dispatch.
    pub async fn request(&mut self, method: &str, params: &str) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;
        let request = parse::build_request(id, method, params);
        self.write(&request).await?;
        {
            let mut guard = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            guard.insert(id, method.to_string());
        }
        Ok(id)
    }

    /// Send a notification to the server.
    async fn send_notification(&self, method: &str, params: &str) -> Result<(), String> {
        let msg = format!(r#"{{"jsonrpc":"2.0","method":"{}","params":{}}}"#, method, params);
        self.write(&msg).await
    }

    /// Write a raw JSON-RPC message to the server's stdin.
    pub async fn write(&self, msg: &str) -> Result<(), String> {
        let header = format!("Content-Length: {}\r\n\r\n", msg.len());
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(header.as_bytes()).await
            .map_err(|e| format!("LSP write error: {}", e))?;
        stdin.write_all(msg.as_bytes()).await
            .map_err(|e| format!("LSP write error: {}", e))?;
        stdin.flush().await
            .map_err(|e| format!("LSP flush error: {}", e))
    }
}

/// Read loop: parse LSP messages from stdout and dispatch events.
async fn read_loop(
    stdout: tokio::process::ChildStdout,
    bg: crate::kernel::runtime::BackgroundHandle,
    language: &str,
    pending: PendingMap,
) {
    let mut reader = BufReader::new(stdout);
    let mut buf = Vec::new();
    let lang = language.to_string();

    loop {
        buf.clear();

        let mut header = String::new();
        loop {
            header.clear();
            match reader.read_line(&mut header).await {
                Ok(0) => return,
                Ok(_) => {
                    let trimmed = header.trim();
                    if trimmed.is_empty() {
                        break;
                    }
                    buf.push(trimmed.to_string());
                }
                Err(_) => return,
            }
        }

        let content_length: usize = buf.iter()
            .find_map(|line| {
                line.strip_prefix("Content-Length: ")
                    .and_then(|s| s.trim().parse().ok())
            })
            .unwrap_or(0);

        if content_length == 0 {
            continue;
        }

        let mut body = vec![0u8; content_length];
        if reader.read_exact(&mut body).await.is_err() {
            return;
        }

        let body_str = String::from_utf8_lossy(&body).to_string();

        // Check for notifications (have "method" but not "id" as top-level key)
        if let Some(method) = parse::extract_method(&body_str) {
            match method.as_str() {
                "textDocument/publishDiagnostics" => {
                    if let Some((uri, diags)) = parse::parse_diagnostics(&body_str) {
                        bg.send_event(BackgroundEvent::LspDiagnostics { path: uri, diagnostics: diags });
                    }
                }
                "$/progress" => {
                    if let Some((token, message, percentage)) = parse::parse_progress(&body_str) {
                        bg.send_event(BackgroundEvent::LspProgress { token, message, percentage });
                    }
                }
                _ => {}
            }
        } else if let Some(id) = parse::extract_id(&body_str) {
            let method = {
                let mut guard = pending.lock().unwrap_or_else(|e| e.into_inner());
                guard.remove(&id)
            };
            let method = match method {
                Some(m) => m,
                None => continue,
            };

            if parse::has_error(&body_str) {
                if let Some(msg) = parse::extract_error(&body_str) {
                    bg.send_event(BackgroundEvent::LspResponse {
                        path: lang.clone(),
                        method: method.clone(),
                        result: msg,
                        request_id: id,
                    });
                }
                continue;
            }

            match method.as_str() {
                "textDocument/hover" => {
                    if let Some(contents) = parse::parse_hover(&body_str) {
                        bg.send_event(BackgroundEvent::LspHover { path: lang.clone(), contents });
                    }
                }
                "textDocument/definition" => {
                    if let Some((uri, sl, sc, el, ec)) = parse::parse_definition(&body_str) {
                        bg.send_event(BackgroundEvent::LspDefinition {
                            path: lang.clone(), uri, start_line: sl, start_col: sc,
                            end_line: el, end_col: ec,
                        });
                    }
                }
                "textDocument/codeAction" => {
                    if let Some(actions) = parse::parse_code_actions(&body_str) {
                        bg.send_event(BackgroundEvent::LspCodeActions {
                            path: lang.clone(),
                            actions: serde_json::to_string(&actions).unwrap_or_default(),
                        });
                    }
                }
                "textDocument/completion" => {
                    if let Some(items) = parse::parse_completion_items(&body_str) {
                        bg.send_event(BackgroundEvent::LspCompletionItems {
                            path: lang.clone(),
                            items: serde_json::to_string(&items).unwrap_or_default(),
                        });
                    }
                }
                "textDocument/rename" => {
                    if let Some(edit) = parse::parse_rename(&body_str) {
                        bg.send_event(BackgroundEvent::LspRenameResult {
                            path: lang.clone(), edit,
                        });
                    }
                }
                _ => {
                    bg.send_event(BackgroundEvent::LspResponse {
                        path: lang.clone(),
                        method,
                        result: body_str,
                        request_id: id,
                    });
                }
            }
        }
    }
}

/// Manages all active LSP server connections.
pub struct LspManager {
    servers: Vec<(String, LspServer)>,
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LspManager {
    pub fn new() -> Self {
        LspManager { servers: Vec::new() }
    }

    pub async fn start_server(
        &mut self,
        command: &str,
        args: &[String],
        language: &str,
        bg: crate::kernel::runtime::BackgroundHandle,
    ) -> Result<(), String> {
        if self.servers.iter().any(|(l, _)| l == language) {
            return Ok(());
        }
        let server = LspServer::start(command, args, language, bg).await?;
        self.servers.push((language.to_string(), server));
        Ok(())
    }

    pub fn server_for(&mut self, language: &str) -> Option<&mut LspServer> {
        self.servers.iter_mut()
            .find(|(l, _)| l == language)
            .map(|(_, s)| s)
    }

    pub fn languages(&self) -> Vec<String> {
        self.servers.iter().map(|(l, _)| l.clone()).collect()
    }
}
