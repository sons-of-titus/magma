use std::collections::HashMap;
use kernel::command::{self, builtin};
use kernel::storage::disk::DiskFileSystem;
use kernel::state::Editor;

#[macro_export]
macro_rules! janet_test {
    ($ed:ident, $body:block) => {{
        let _lock = scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut $ed = helpers::make_editor();
        scripting::init(&mut $ed);
        let _editor_scope = scripting::EditorScope::new(&mut $ed);
        $body
    }};
}

pub fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer("test");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

pub fn make_editor_with_buffer(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer_from_str("test", content);
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

pub fn run(ed: &mut Editor, cmd: &str) {
    command::execute_command(ed, cmd, &HashMap::new()).unwrap();
}

pub fn focused_key(ed: &Editor) -> usize {
    ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .unwrap_or(0)
}

pub fn buf_text(ed: &Editor) -> String {
    let key = focused_key(ed);
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    buf.slice(0, buf.len())
}

pub fn cursor(ed: &Editor) -> usize {
    let key = focused_key(ed);
    ed.views.get(&key).unwrap().cursor_offset()
}

#[cfg(feature = "janet")]
pub fn acquire_janet_lock() -> std::sync::MutexGuard<'static, ()> {
    scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// A minimal DAP adapter that answers `initialize` and appends every request
/// command it receives to a log file — the observability the ordering tests
/// need without a real debugger.
const FAKE_DAP_ADAPTER: &str = r#"#!/usr/bin/env python3
import sys, json
LOG_PATH = "__LOG_PATH__"
log = open(LOG_PATH, "w")
seq = 0
def send(msg):
    global seq
    msg["seq"] = seq; seq += 1
    body = json.dumps(msg).encode()
    sys.stdout.buffer.write(b"Content-Length: %d\r\n\r\n%s" % (len(body), body))
    sys.stdout.buffer.flush()
def read_message():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if line in (b"\r\n", b"\n"):
            break
        k, _, v = line.decode().partition(":")
        headers[k.strip().lower()] = v.strip()
    length = int(headers["content-length"])
    return json.loads(sys.stdin.buffer.read(length))
while True:
    try:
        msg = read_message()
    except Exception:
        break
    log.write(msg.get("command", "?") + "\n")
    if "arguments" in msg:
        log.write("\t" + json.dumps(msg["arguments"]) + "\n")
    log.flush()
    if msg.get("command") == "initialize":
        send({"type": "response", "request_seq": msg["seq"], "success": True,
              "command": "initialize", "body": {}})
    elif msg.get("command") == "threads":
        send({"type": "response", "request_seq": msg["seq"], "success": True,
              "command": "threads",
              "body": {"threads": [{"id": 1, "name": "main"},
                                   {"id": 2, "name": "worker"}]}})
"#;

pub fn python3_available() -> bool {
    std::process::Command::new("python3").arg("-c").arg("1").output().is_ok()
}

pub struct FakeDap {
    pub script: std::path::PathBuf,
    pub log: std::path::PathBuf,
}

pub fn spawn_fake_dap(tag: &str) -> FakeDap {
    let dir = std::env::temp_dir().join(format!("magma_fake_dap_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake_adapter.py");
    let log = dir.join("requests.log");
    let text = FAKE_DAP_ADAPTER.replace("__LOG_PATH__", &log.to_string_lossy());
    std::fs::write(&script, text).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    FakeDap { script, log }
}

pub fn dap_log_lines(log: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .map(|t| t.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

pub fn editor_with_background() -> (Editor, kernel::runtime::BackgroundHandle,
    tokio::sync::mpsc::UnboundedReceiver<kernel::runtime::BackgroundEvent>) {
    let mut ed = make_editor();
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = kernel::runtime::BackgroundHandle::new(runtime, tx);
    ed.background = Some(handle.clone());
    (ed, handle, rx)
}
