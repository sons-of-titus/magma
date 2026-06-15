use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::LazyLock;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use serde::{Deserialize, Serialize};

use crate::state::Editor;

/// Broadcast channel for peer events (cursor moves, buffer inserts, etc.).
/// All subscribers receive every message sent on this channel.
static PEER_TX: LazyLock<broadcast::Sender<String>> =
    LazyLock::new(|| broadcast::channel::<String>(256).0);

/// Listen for TCP connections on `addr`, handling editor commands
/// via a JSON-line protocol.
pub async fn run_server(editor: Arc<RwLock<Editor>>, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(addr).await?;
    debug!("Magma server listening on {addr}");

    loop {
        let (stream, peer) = listener.accept().await?;
        debug!("Connection from {peer}");
        let ed = editor.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(ed, stream).await {
                debug!("Client {peer} error: {e}");
            }
        });
    }
}

async fn handle_client(editor: Arc<RwLock<Editor>>, stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    // Peer event subscription — activated by the "subscribe-events" command.
    let mut peer_rx: Option<broadcast::Receiver<String>> = None;

    loop {
        let peer_rx_ref = &mut peer_rx;
        let event = tokio::select! {
            line = lines.next_line() => {
                match line? {
                    Some(l) => Either::Line(l),
                    None => break,
                }
            }
            ev = async {
                match peer_rx_ref {
                    Some(rx) => match rx.recv().await {
                        Ok(msg) => Some(msg),
                        Err(_) => None,
                    },
                    None => {
                        std::future::pending::<Option<String>>().await
                    }
                }
            } => {
                match ev {
                    Some(json) => Either::PeerEvent(json),
                    None => continue,
                }
            }
        };

        match event {
            Either::PeerEvent(json) => {
                writer.write_all(format!("{json}\n").as_bytes()).await?;
            }
            Either::Line(raw_line) => {
                let line = raw_line.trim().to_string();
                if line.is_empty() { continue; }

                let request: Request = match serde_json::from_str(&line) {
                    Ok(r) => r,
                    Err(e) => {
                        let resp = Response::error(&format!("Invalid JSON: {e}"), 0);
                        let _ = writer.write_all(&resp.to_json().into_bytes()).await;
                        let _ = writer.write_all(b"\n").await;
                        continue;
                    }
                };

                let response = match request.cmd.as_str() {
                    "ping" => Response::ok(serde_json::json!("pong"), request.id),
                    "open-file" => handle_open_file(&editor, &request).await,
                    "get-buffer" => handle_get_buffer(&editor, &request),
                    "insert" => {
                        let resp = handle_insert(&editor, &request);
                        if resp.ok {
                            // Broadcast peer insert event to subscribed clients
                            let buf_id = request.args.get("buffer").and_then(|v| v.as_u64()).unwrap_or(0);
                            let pos = request.args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);
                            let text = request.args.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let ev = serde_json::json!({
                                "event": "peer-insert",
                                "buffer": buf_id,
                                "offset": pos,
                                "text": text,
                            }).to_string();
                            let _ = PEER_TX.send(ev);
                        }
                        resp
                    }
                    "delete" => {
                        let resp = handle_delete(&editor, &request);
                        if resp.ok {
                            let buf_id = request.args.get("buffer").and_then(|v| v.as_u64()).unwrap_or(0);
                            let start = request.args.get("start").and_then(|v| v.as_u64()).unwrap_or(0);
                            let end = request.args.get("end").and_then(|v| v.as_u64()).unwrap_or(0);
                            let ev = serde_json::json!({
                                "event": "peer-delete",
                                "buffer": buf_id,
                                "start": start,
                                "end": end,
                            }).to_string();
                            let _ = PEER_TX.send(ev);
                        }
                        resp
                    }
                    "save" => handle_save(&editor, &request),
                    "eval" => handle_eval(&editor, &request),
                    "close" => handle_close(&editor, &request),
                    "list-buffers" => handle_list_buffers(&editor, &request),
                    "subscribe-events" => {
                        peer_rx = Some(PEER_TX.subscribe());
                        Response::ok(serde_json::json!("subscribed"), request.id)
                    }
                    "peer-cursor" => {
                        let buf_id = request.args.get("buffer").and_then(|v| v.as_u64()).unwrap_or(0);
                        let offset = request.args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);
                        let ev = serde_json::json!({
                            "event": "peer-cursor",
                            "buffer": buf_id,
                            "offset": offset,
                        }).to_string();
                        let _ = PEER_TX.send(ev);
                        Response::ok(serde_json::json!("broadcast"), request.id)
                    }
                    _ => Response::error(&format!("Unknown command: {}", request.cmd), request.id),
                };

                let out = response.to_json();
                writer.write_all(out.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
        }
    }

    Ok(())
}

/// Internal discriminant to break `tokio::select!` into typed arms.
enum Either {
    Line(String),
    PeerEvent(String),
}

// ── Command handlers ──────────────────────────────────────────────────────────

async fn handle_open_file(editor: &Arc<RwLock<Editor>>, req: &Request) -> Response {
    let path = match req.args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => return Response::error("Missing 'path' argument", req.id),
    };

    let content = match tokio::task::spawn_blocking({
        let path = path.clone();
        move || std::fs::read_to_string(&path)
    }).await {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => return Response::error(&format!("Cannot read {path}: {e}"), req.id),
        Err(e) => return Response::error(&format!("Task error: {e}"), req.id),
    };

    let ed = editor.read().unwrap_or_else(|e| e.into_inner());
    // Check if already open
    for (key, buf) in ed.buffers.iter() {
        if buf.path.as_deref() == Some(&path) {
            let result = serde_json::json!({
                "buffer": key,
                "name": buf.name,
                "path": buf.path,
                "size": buf.len(),
            });
            return Response::ok(result, req.id);
        }
    }
    drop(ed);

    // Open new buffer
    let name = std::path::Path::new(&path)
        .file_name().map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());

    let mut ed = editor.write().unwrap_or_else(|e| e.into_inner());
    let id = ed.allocate_buffer_id();
    let mut buf = crate::buffer::Buffer::from_string(
        crate::state::id::BufferId(id), &name, &content,
    );
    buf.path = Some(path);
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);

    let result = serde_json::json!({
        "buffer": key,
        "name": name,
        "size": content.len(),
    });
    Response::ok(result, req.id)
}

fn handle_get_buffer(editor: &Arc<RwLock<Editor>>, req: &Request) -> Response {
    let buf_id = match req.args.get("buffer").and_then(|v| v.as_u64()).map(|v| v as usize) {
        Some(id) => id,
        None => return Response::error("Missing 'buffer' argument", req.id),
    };

    let ed = editor.read().unwrap_or_else(|e| e.into_inner());
    match ed.buffers.get(buf_id) {
        Some(buf) => {
            let content = buf.slice(0, buf.len());
            let result = serde_json::json!({
                "content": content,
                "name": buf.name,
                "path": buf.path,
                "size": buf.len(),
                "cursor": buf.cursor(),
            });
            Response::ok(result, req.id)
        }
        None => Response::error(&format!("Buffer {buf_id} not found"), req.id),
    }
}

fn handle_insert(editor: &Arc<RwLock<Editor>>, req: &Request) -> Response {
    let buf_id = match req.args.get("buffer").and_then(|v| v.as_u64()) {
        Some(id) => id as usize,
        None => return Response::error("Missing 'buffer' argument", req.id),
    };
    let text = match req.args.get("text").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => return Response::error("Missing 'text' argument", req.id),
    };
    let offset = req.args.get("offset").and_then(|v| v.as_u64()).map(|v| v as usize);

    let mut ed = editor.write().unwrap_or_else(|e| e.into_inner());
    match ed.buffers.get_mut(buf_id) {
        Some(buf) => {
            let pos = offset.unwrap_or_else(|| buf.cursor());
            buf.insert(pos, &text);
            Response::ok(serde_json::json!({"cursor": buf.cursor(), "size": buf.len()}), req.id)
        }
        None => Response::error(&format!("Buffer {buf_id} not found"), req.id),
    }
}

fn handle_delete(editor: &Arc<RwLock<Editor>>, req: &Request) -> Response {
    let buf_id = match req.args.get("buffer").and_then(|v| v.as_u64()) {
        Some(id) => id as usize,
        None => return Response::error("Missing 'buffer' argument", req.id),
    };
    let start = match req.args.get("start").and_then(|v| v.as_u64()) {
        Some(v) => v as usize,
        None => return Response::error("Missing 'start' argument", req.id),
    };
    let end = match req.args.get("end").and_then(|v| v.as_u64()) {
        Some(v) => v as usize,
        None => return Response::error("Missing 'end' argument", req.id),
    };

    let mut ed = editor.write().unwrap_or_else(|e| e.into_inner());
    match ed.buffers.get_mut(buf_id) {
        Some(buf) => {
            buf.delete(start, end);
            Response::ok(serde_json::json!({"cursor": buf.cursor(), "size": buf.len()}), req.id)
        }
        None => Response::error(&format!("Buffer {buf_id} not found"), req.id),
    }
}

fn handle_save(editor: &Arc<RwLock<Editor>>, req: &Request) -> Response {
    let buf_id = match req.args.get("buffer").and_then(|v| v.as_u64()) {
        Some(id) => id as usize,
        None => return Response::error("Missing 'buffer' argument", req.id),
    };

    let ed = editor.read().unwrap_or_else(|e| e.into_inner());
    match ed.buffers.get(buf_id) {
        Some(buf) => {
            let path = match &buf.path {
                Some(p) => p.clone(),
                None => return Response::error("Buffer has no path", req.id),
            };
            let content = buf.slice(0, buf.len());
            drop(ed);

            if let Err(e) = std::fs::write(&path, &content) {
                return Response::error(&format!("Save failed: {e}"), req.id);
            }
            let mut ed = editor.write().unwrap_or_else(|e| e.into_inner());
            if let Some(buf) = ed.buffers.get_mut(buf_id) {
                buf.mark_saved();
            }
            Response::ok(serde_json::json!({"path": path}), req.id)
        }
        None => Response::error(&format!("Buffer {buf_id} not found"), req.id),
    }
}

fn handle_eval(editor: &Arc<RwLock<Editor>>, req: &Request) -> Response {
    let code = match req.args.get("code").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => return Response::error("Missing 'code' argument", req.id),
    };

    #[cfg(feature = "janet")]
    {
        let mut ed = editor.write().unwrap_or_else(|e| e.into_inner());
        let result = crate::janet_bridge::eval(&mut ed, &code);
        Response::ok(serde_json::json!({"result": result}), req.id)
    }
    #[cfg(not(feature = "janet"))]
    {
        let _ = editor;
        Response::error("Janet not enabled", req.id)
    }
}

fn handle_close(editor: &Arc<RwLock<Editor>>, req: &Request) -> Response {
    let buf_id = match req.args.get("buffer").and_then(|v| v.as_u64()) {
        Some(id) => id as usize,
        None => return Response::error("Missing 'buffer' argument", req.id),
    };

    let mut ed = editor.write().unwrap_or_else(|e| e.into_inner());
    if ed.buffers.contains(buf_id) {
        ed.buffers.remove(buf_id);
        Response::ok(serde_json::json!("closed"), req.id)
    } else {
        Response::error(&format!("Buffer {buf_id} not found"), req.id)
    }
}

fn handle_list_buffers(editor: &Arc<RwLock<Editor>>, req: &Request) -> Response {
    let ed = editor.read().unwrap_or_else(|e| e.into_inner());
    let buffers: Vec<serde_json::Value> = ed.buffers.iter().map(|(key, buf)| {
        serde_json::json!({
            "id": key,
            "name": buf.name,
            "path": buf.path,
            "size": buf.len(),
            "modified": buf.modified(),
        })
    }).collect();
    Response::ok(serde_json::json!(buffers), req.id)
}

// ── Protocol types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Request {
    cmd: String,
    #[serde(default)]
    args: HashMap<String, serde_json::Value>,
    #[serde(default)]
    id: u64,
}

#[derive(Debug, Serialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    id: u64,
}

impl Response {
    fn ok(result: serde_json::Value, id: u64) -> Self {
        Response { ok: true, result: Some(result), error: None, id }
    }

    fn error(msg: &str, id: u64) -> Self {
        Response { ok: false, result: None, error: Some(msg.to_string()), id }
    }

    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| r#"{"ok":false,"error":"serialization error","id":0}"#.to_string())
    }
}
