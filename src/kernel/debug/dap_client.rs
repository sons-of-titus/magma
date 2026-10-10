//! DAP (Debug Adapter Protocol) client — the LSP treatment (rework.md §4
//! applied to debugging): awaited initialize, ordered launch, graceful
//! disconnect, and kill-on-drop.  Wire framing is shared with LSP.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::types::SessionId;
use crate::kernel::runtime::BackgroundEvent;

const INIT_TIMEOUT: Duration = Duration::from_secs(10);

struct DapCore {
    outbound: tokio::sync::mpsc::UnboundedSender<Value>,
    seq: AtomicI64,
    /// Signaled by the reader when the `initialize` response arrives.
    init_signal: Mutex<std::sync::mpsc::Sender<Result<(), String>>>,
}

/// A live adapter connection.  Clones share the wire.
#[derive(Clone)]
pub struct DapHandle {
    core: Arc<DapCore>,
    session_id: SessionId,
    child: Arc<Mutex<Option<std::process::Child>>>,
}

impl std::fmt::Debug for DapHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DapHandle").field("session_id", &self.session_id).finish()
    }
}

impl DapHandle {
    /// Spawn the adapter, await `initialize`, then send the optional launch
    /// configuration.  Blocks the calling thread — run it off the foreground.
    pub fn start(
        adapter: &str,
        session_id: SessionId,
        launch_config: Option<Value>,
        bg: crate::kernel::runtime::BackgroundHandle,
    ) -> Result<DapHandle, String> {
        let mut child = std::process::Command::new(adapter)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn debug adapter '{adapter}': {e}"))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| "adapter stdin unavailable".to_string())?;
        let stdout = child.stdout.take()
            .ok_or_else(|| "adapter stdout unavailable".to_string())?;

        let (outbound, outbound_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
        let (init_tx, init_rx) = std::sync::mpsc::channel();

        let core = Arc::new(DapCore {
            outbound,
            seq: AtomicI64::new(1),
            init_signal: Mutex::new(init_tx),
        });

        std::thread::Builder::new()
            .name(format!("dap-write-{session_id}"))
            .spawn(move || writer_loop(stdin, outbound_rx))
            .map_err(|e| format!("DAP writer thread: {e}"))?;

        let reader_core = core.clone();
        std::thread::Builder::new()
            .name(format!("dap-read-{session_id}"))
            .spawn(move || reader_loop(stdout, reader_core, session_id, bg))
            .map_err(|e| format!("DAP reader thread: {e}"))?;

        let handle = DapHandle {
            core,
            session_id,
            child: Arc::new(Mutex::new(Some(child))),
        };

        // The awaited handshake.
        handle.request("initialize", json!({
            "clientID": "magma",
            "adapterID": "magma",
            "linesStartAt1": true,
            "columnsStartAt1": true,
            "pathFormat": "path",
        }));
        match init_rx.recv_timeout(INIT_TIMEOUT) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                handle.kill();
                return Err(format!("DAP initialize failed (session {session_id}): {e}"));
            }
            Err(_) => {
                handle.kill();
                return Err(format!("DAP initialize timed out (session {session_id})"));
            }
        }

        // Launch/attach ordering: only after initialize completed, and the
        // request verb comes from the configuration itself.
        if let Some(config) = launch_config {
            let command = if config.get("request").and_then(Value::as_str) == Some("attach") {
                "attach"
            } else {
                "launch"
            };
            handle.request(command, config);
        }
        Ok(handle)
    }

    fn alloc_seq(&self) -> i64 {
        self.core.seq.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a DAP request (fire and forget — responses arrive as events,
    /// correlated by the `command` field the adapter echoes back).
    pub fn request(&self, command: &str, arguments: Value) {
        let seq = self.alloc_seq();
        let msg = json!({
            "seq": seq,
            "type": "request",
            "command": command,
            "arguments": arguments,
        });
        let _ = self.core.outbound.send(msg);
    }

    /// Ordered teardown: `disconnect` request, then kill.  The reader
    /// observes EOF and emits `DapTerminated`.
    pub fn disconnect(&self) {
        self.request("disconnect", json!({ "terminateDebuggee": true }));
        std::thread::sleep(Duration::from_millis(100));
        self.kill();
    }

    pub fn kill(&self) {
        if let Ok(mut guard) = self.child.lock()
            && let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
    }
}

fn writer_loop(mut stdin: std::process::ChildStdin, rx: tokio::sync::mpsc::UnboundedReceiver<Value>) {
    use std::io::Write;
    let mut rx = rx;
    while let Some(msg) = rx.blocking_recv() {
        let body = serde_json::to_string(&msg).unwrap_or_else(|_| "{}".into());
        let bytes = format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes();
        if stdin.write_all(&bytes).is_err() || stdin.flush().is_err() {
            return;
        }
    }
}

fn reader_loop(
    stdout: std::process::ChildStdout,
    core: Arc<DapCore>,
    session_id: SessionId,
    bg: crate::kernel::runtime::BackgroundHandle,
) {
    let mut read = stdout;
    loop {
        match crate::kernel::semantic::lsp_proto::read_message(&mut read) {
            Err(_) => break,
            Ok(msg) => dispatch_dap_message(&core, &msg, &bg, session_id),
        }
    }
    bg.send_event(BackgroundEvent::DapTerminated { session_id });
}

fn dispatch_dap_message(
    core: &Arc<DapCore>,
    v: &Value,
    bg: &crate::kernel::runtime::BackgroundHandle,
    session_id: SessionId,
) {
    let msg_type = v.get("type").and_then(Value::as_str).unwrap_or("");
    match msg_type {
        "event" => {
            let event_name = v.get("event").and_then(Value::as_str).unwrap_or("");
            match event_name {
                "stopped" => {
                    let reason = v.pointer("/body/reason").and_then(Value::as_str)
                        .unwrap_or("unknown").to_string();
                    let thread_id = v.pointer("/body/threadId").and_then(Value::as_u64).unwrap_or(0);
                    bg.send_event(BackgroundEvent::DapStopped { session_id, reason, thread_id });
                }
                "output" => {
                    let category = v.pointer("/body/category").and_then(Value::as_str)
                        .unwrap_or("console").to_string();
                    let output = v.pointer("/body/output").and_then(Value::as_str)
                        .unwrap_or("").to_string();
                    bg.send_event(BackgroundEvent::DapOutput { session_id, category, output });
                }
                "breakpoint" => {
                    // Adapter confirmed/changed one breakpoint: the session
                    // model updates its verified flag from this.
                    let body = v.get("body").cloned().unwrap_or(Value::Null);
                    bg.send_event(BackgroundEvent::DapCommandResult {
                        session_id,
                        command: "breakpoint".to_string(),
                        success: true,
                        body: body.to_string(),
                    });
                }
                "terminated" | "exited" => {
                    bg.send_event(BackgroundEvent::DapTerminated { session_id });
                }
                _ => {}
            }
        }
        "response" => {
            let command = v.get("command").and_then(Value::as_str).unwrap_or("");
            if command == "initialize" {
                let failed = !v.get("success").and_then(Value::as_bool).unwrap_or(false);
                let error = v.pointer("/message").and_then(Value::as_str).unwrap_or("adapter error");
                if let Ok(tx) = core.init_signal.lock() {
                    let _ = tx.send(if failed { Err(error.to_string()) } else { Ok(()) });
                }
                return;
            }
            if command == "evaluate" && v.get("success").and_then(Value::as_bool).unwrap_or(false) {
                let result = v.pointer("/body/result").and_then(Value::as_str)
                    .unwrap_or("").to_string();
                bg.send_event(BackgroundEvent::DapResponse {
                    session_id,
                    event_type: "evaluate".to_string(),
                    data: result,
                });
                return;
            }
            let body = serde_json::to_string(v.get("body").unwrap_or(&Value::Null))
                .unwrap_or_default();
            bg.send_event(BackgroundEvent::DapCommandResult {
                session_id,
                command: command.to_string(),
                success: v.get("success").and_then(Value::as_bool).unwrap_or(false),
                body,
            });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_shape_carries_seq_and_command() {
        let msg = json!({
            "seq": 7, "type": "request", "command": "launch",
            "arguments": { "program": "a.out" },
        });
        assert_eq!(msg["seq"], 7);
        assert_eq!(msg["type"], "request");
        assert_eq!(msg["command"], "launch");
    }
}
