//! Headless server — a foreground loop owns the editor and applies ops
//! arriving from TCP clients over a channel (rework.md §1: one owner, no
//! global lock).
//!
//! The protocol is JSON lines: `{cmd, args, id}` → `{ok, result|error, id}`.
//! The former peer-broadcast layer was deleted: it echoed raw byte offsets
//! to remote peers with no transformation, which corrupts documents the
//! moment two writers race — a façade of collaboration, not a feature.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::runtime::BackgroundEvent;
use crate::state::Editor;

/// One editor operation requested by a client.  File content is read on the
/// async side; the foreground loop only ever touches in-memory state.
enum ServerOp {
    Ping,
    OpenFile {
        path: String,
        content: String,
        id: u64,
    },
    GetBuffer {
        buf_id: usize,
        id: u64,
    },
    Insert {
        buf_id: usize,
        text: String,
        offset: Option<usize>,
        id: u64,
    },
    Delete {
        buf_id: usize,
        start: usize,
        end: usize,
        id: u64,
    },
    Save {
        buf_id: usize,
        id: u64,
    },
    Eval {
        code: String,
        id: u64,
    },
    Close {
        buf_id: usize,
        id: u64,
    },
    ListBuffers {
        id: u64,
    },
}

type Responder = tokio::sync::oneshot::Sender<Response>;

/// Listen on `addr`.  A dedicated thread owns the editor; every TCP handler
/// proxies ops to it and replies through a oneshot.
pub async fn run_server(
    editor: Editor,
    bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>,
    addr: &str,
    script_runtime: Option<Box<dyn crate::script_runtime::ScriptRuntime>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(addr).await?;
    // Every op — eval executes arbitrary Janet — requires the bearer
    // token.  It is random per run (or MAGMA_SERVER_TOKEN when set) and
    // printed once to stderr for the client side.
    let token = std::env::var("MAGMA_SERVER_TOKEN").unwrap_or_else(|_| {
        use std::fmt::Write as _;
        let mut buf = [0u8; 24];
        getrandom_fill(&mut buf);
        let mut token = String::with_capacity(48);
        for b in buf {
            logging::LogErr::log_err(write!(token, "{b:02x}"));
        }
        token
    });
    info!("Magma server token: {token}");
    debug!("Magma server listening on {addr}");

    let (op_tx, op_rx) = mpsc::unbounded_channel::<(ServerOp, Responder)>();
    std::thread::Builder::new()
        .name("magma-server-foreground".into())
        .spawn(move || foreground_loop(editor, bg_receiver, op_rx, script_runtime))?;

    loop {
        let (stream, peer) = listener.accept().await?;
        debug!("Connection from {peer}");
        let op_tx = op_tx.clone();
        let token = token.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(op_tx, stream, &token).await {
                debug!("Client {peer} error: {e}");
            }
        });
    }
}

/// The server's single owner of editor state.  Janet is initialized on this
/// thread and never leaves it.
fn foreground_loop(
    mut ed: Editor,
    mut bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>,
    op_rx: mpsc::UnboundedReceiver<(ServerOp, Responder)>,
    mut script_runtime: Option<Box<dyn crate::script_runtime::ScriptRuntime>>,
) {
    if let Some(rt) = script_runtime.take() {
        let mut rt = rt;
        rt.init(&mut ed);
        ed.runtime = Some(rt);
    }

    let mut op_rx = op_rx;
    loop {
        while let Ok(event) = bg_receiver.try_recv() {
            crate::runtime::process_background_event(&mut ed, event);
        }
        ed.publish_buffer_changes();
        ed.dispatch_events_scoped();

        let Some((op, responder)) = op_rx.blocking_recv() else {
            return;
        };
        let response = apply_op(&mut ed, op);
        let _ = responder.send(response);
    }
}

fn apply_op(ed: &mut Editor, op: ServerOp) -> Response {
    match op {
        ServerOp::Ping => Response::ok(serde_json::json!("pong"), 0),
        ServerOp::OpenFile { path, content, id } => {
            for (key, arc) in ed.buffers.iter() {
                let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                if buf.path.as_deref() == Some(&path) {
                    let result = serde_json::json!({
                        "buffer": key,
                        "name": buf.name.clone(),
                        "path": buf.path.clone(),
                        "size": buf.len(),
                    });
                    return Response::ok(result, id);
                }
            }
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            let buf_id = ed.allocate_buffer_id();
            let mut buf = crate::text_engine::Buffer::from_string(
                crate::state::id::BufferId(buf_id),
                &name,
                &content,
            );
            buf.path = Some(path);
            let arc = std::sync::Arc::new(std::sync::Mutex::new(buf));
            let entry = ed.buffers.vacant_entry();
            let key = entry.key();
            entry.insert(arc.clone());
            ed.views
                .insert(key, crate::text_engine::BufferView::new(arc));
            Response::ok(
                serde_json::json!({"buffer": key, "name": name, "size": content.len()}),
                id,
            )
        }
        ServerOp::GetBuffer { buf_id, id } => {
            let Some(arc) = ed.buffers.get(buf_id) else {
                return Response::error(&format!("Buffer {buf_id} not found"), id);
            };
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            let result = serde_json::json!({
                "content": buf.slice(0, buf.len()),
                "name": buf.name.clone(),
                "path": buf.path.clone(),
                "size": buf.len(),
                "cursor": ed.view_of_buffer(buf_id).map(|v| v.cursor_offset()).unwrap_or(0),
            });
            Response::ok(result, id)
        }
        ServerOp::Insert {
            buf_id,
            text,
            offset,
            id,
        } => {
            if !ed.buffers.contains(buf_id) {
                return Response::error(&format!("Buffer {buf_id} not found"), id);
            }
            let pos = offset.unwrap_or_else(|| {
                ed.view_of_buffer(buf_id)
                    .map(|v| v.cursor_offset())
                    .unwrap_or(0)
            });
            if let Some(view) = ed.view_of_buffer_mut(buf_id) {
                view.insert(pos, &text);
            }
            let cursor = ed
                .view_of_buffer(buf_id)
                .map(|v| v.cursor_offset())
                .unwrap_or(0);
            let size = ed
                .buffers
                .get(buf_id)
                .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).len())
                .unwrap_or(0);
            Response::ok(serde_json::json!({"cursor": cursor, "size": size}), id)
        }
        ServerOp::Delete {
            buf_id,
            start,
            end,
            id,
        } => {
            if !ed.buffers.contains(buf_id) {
                return Response::error(&format!("Buffer {buf_id} not found"), id);
            }
            if let Some(view) = ed.view_of_buffer_mut(buf_id) {
                view.delete(start, end);
            }
            let cursor = ed
                .view_of_buffer(buf_id)
                .map(|v| v.cursor_offset())
                .unwrap_or(0);
            let size = ed
                .buffers
                .get(buf_id)
                .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).len())
                .unwrap_or(0);
            Response::ok(serde_json::json!({"cursor": cursor, "size": size}), id)
        }
        ServerOp::Save { buf_id, id } => {
            let Some(arc) = ed.buffers.get(buf_id) else {
                return Response::error(&format!("Buffer {buf_id} not found"), id);
            };
            let (path, content, rev) = {
                let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                let Some(path) = buf.path.clone() else {
                    return Response::error("Buffer has no path", id);
                };
                (path, buf.slice(0, buf.len()), buf.revision())
            };
            // Atomic write happens off the foreground thread; the buffer is
            // marked clean only for the exact revision that was written.
            let write = move || crate::storage::disk::atomic_write(&path, &content);
            match std::thread::spawn(write).join() {
                Ok(Ok(())) => {
                    if let Some(arc) = ed.buffers.get_mut(buf_id) {
                        arc.lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .mark_saved_at(rev);
                    }
                    Response::ok(serde_json::json!({"saved": true}), id)
                }
                Ok(Err(e)) => Response::error(&format!("Save failed: {e}"), id),
                Err(e) => { logging::error!("save task panicked: {e:?}"); Response::error("Save task panicked", id) }
            }
        }
        ServerOp::Eval { code, id } => {
            let mut rt = ed.runtime.take();
            let result = match &mut rt {
                Some(rt) => rt.eval(ed, &code),
                None => "Scripting runtime not available".to_string(),
            };
            ed.runtime = rt;
            Response::ok(serde_json::json!({"result": result}), id)
        }
        ServerOp::Close { buf_id, id } => {
            if ed.buffers.contains(buf_id) {
                ed.drop_buffer(buf_id);
                Response::ok(serde_json::json!("closed"), id)
            } else {
                Response::error(&format!("Buffer {buf_id} not found"), id)
            }
        }
        ServerOp::ListBuffers { id } => {
            let buffers: Vec<serde_json::Value> = ed
                .buffers
                .iter()
                .map(|(key, arc)| {
                    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                    serde_json::json!({
                        "id": key,
                        "name": buf.name.clone(),
                        "path": buf.path.clone(),
                        "size": buf.len(),
                        "modified": buf.modified(),
                    })
                })
                .collect();
            Response::ok(serde_json::json!(buffers), id)
        }
    }
}

async fn handle_client(
    op_tx: mpsc::UnboundedSender<(ServerOp, Responder)>,
    stream: TcpStream,
    token: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::time::{Duration, timeout};
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // First line must be the auth handshake; no ops before it.
    let first = timeout(Duration::from_secs(10), lines.next_line()).await??;
    match first {
        Some(l) if l.trim() == format!("auth {token}") => {}
        _ => {
            writer
                .write_all(
                    Response::error("unauthorized: expected 'auth <token>' handshake", 0)
                        .to_json()
                        .as_bytes(),
                )
                .await?;
            writer.write_all(b"\n").await?;
            return Ok(());
        }
    }

    while let Some(raw) = lines.next_line().await? {
        let line = raw.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response::error(&format!("Invalid JSON: {e}"), 0);
                writer.write_all(resp.to_json().as_bytes()).await?;
                writer.write_all(b"\n").await?;
                continue;
            }
        };

        let op = match translate_op(&request).await {
            Ok(op) => op,
            Err(resp) => {
                writer.write_all(resp.to_json().as_bytes()).await?;
                writer.write_all(b"\n").await?;
                continue;
            }
        };

        let (responder, receipt) = tokio::sync::oneshot::channel();
        let id = request.id;
        if op_tx.send((op, responder)).is_err() {
            writer
                .write_all(
                    Response::error("server shutting down", id)
                        .to_json()
                        .as_bytes(),
                )
                .await?;
            writer.write_all(b"\n").await?;
            continue;
        }
        let response = match receipt.await {
            Ok(r) => r,
            Err(e) => { logging::error!("mod.rs:370: {e}"); Response::error("foreground loop dropped the request", id) }
        };
        writer.write_all(response.to_json().as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }

    Ok(())
}

/// Translate a protocol request into a foreground op.  Argument validation
/// and the file read (async context) happen here, never on the foreground
/// thread.
async fn translate_op(request: &Request) -> Result<ServerOp, Response> {
    let id = request.id;
    let arg_u64 = |name: &str| request.args.get(name).and_then(|v| v.as_u64());
    let arg_str = |name: &str| {
        request
            .args
            .get(name)
            .and_then(|v| v.as_str())
            .map(String::from)
    };

    match request.cmd.as_str() {
        "ping" => Ok(ServerOp::Ping),
        "open-file" => {
            let path =
                arg_str("path").ok_or_else(|| Response::error("Missing 'path' argument", id))?;
            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| Response::error(&format!("Cannot read {path}: {e}"), id))?;
            Ok(ServerOp::OpenFile { path, content, id })
        }
        "get-buffer" => {
            let buf_id = arg_u64("buffer")
                .ok_or_else(|| Response::error("Missing 'buffer' argument", id))?
                as usize;
            Ok(ServerOp::GetBuffer { buf_id, id })
        }
        "insert" => {
            let buf_id = arg_u64("buffer")
                .ok_or_else(|| Response::error("Missing 'buffer' argument", id))?
                as usize;
            let text =
                arg_str("text").ok_or_else(|| Response::error("Missing 'text' argument", id))?;
            let offset = arg_u64("offset").map(|v| v as usize);
            Ok(ServerOp::Insert {
                buf_id,
                text,
                offset,
                id,
            })
        }
        "delete" => {
            let buf_id = arg_u64("buffer")
                .ok_or_else(|| Response::error("Missing 'buffer' argument", id))?
                as usize;
            let start = arg_u64("start")
                .ok_or_else(|| Response::error("Missing 'start' argument", id))?
                as usize;
            let end = arg_u64("end").ok_or_else(|| Response::error("Missing 'end' argument", id))?
                as usize;
            Ok(ServerOp::Delete {
                buf_id,
                start,
                end,
                id,
            })
        }
        "save" => {
            let buf_id = arg_u64("buffer")
                .ok_or_else(|| Response::error("Missing 'buffer' argument", id))?
                as usize;
            Ok(ServerOp::Save { buf_id, id })
        }
        "eval" => {
            let code =
                arg_str("code").ok_or_else(|| Response::error("Missing 'code' argument", id))?;
            Ok(ServerOp::Eval { code, id })
        }
        "close" => {
            let buf_id = arg_u64("buffer")
                .ok_or_else(|| Response::error("Missing 'buffer' argument", id))?
                as usize;
            Ok(ServerOp::Close { buf_id, id })
        }
        "list-buffers" => Ok(ServerOp::ListBuffers { id }),
        other => Err(Response::error(&format!("Unknown command: {other}"), id)),
    }
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
        Response {
            ok: true,
            result: Some(result),
            error: None,
            id,
        }
    }

    fn error(msg: &str, id: u64) -> Self {
        logging::error!("server request {id}: {msg}");
        Response {
            ok: false,
            result: None,
            error: Some(msg.to_string()),
            id,
        }
    }

    fn to_json(&self) -> String {
        serde_json::to_string(self)
            .unwrap_or_else(|_| r#"{"ok":false,"error":"serialization error","id":0}"#.to_string())
    }
}

fn getrandom_fill(buf: &mut [u8]) {
    // The OS CSPRNG never fails on supported platforms; the fallback only
    // exists so a token can never silently become a constant.
    if getrandom::getrandom(buf).is_ok() {
        return;
    }
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for (i, b) in buf.iter_mut().enumerate() {
        *b = ((seed >> ((i % 16) * 8)) & 0xff) as u8 ^ (i as u8);
    }
}
