//! Janet API functions for LSP — registered as `extern "C"` via evil-janet.

use std::sync::{LazyLock, Mutex};

use tokio::io::AsyncWriteExt;

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::input;
use crate::kernel::semantic::client::LspServer;
use crate::kernel::semantic::parse;

type ServerMap = std::collections::HashMap<String, LspServer>;
static SERVERS: LazyLock<Mutex<ServerMap>> = LazyLock::new(|| Mutex::new(ServerMap::new()));

/// Send a JSON-RPC request to an LSP server via a spawned async task.
/// Returns nil; the response is delivered via events (lsp-response, lsp-hover, etc.).
fn send_request(language: &str, method: &str, params: &str) {
    let (stdin_arc, bg, msg, _id, _pending) = {
        let mut guard = match SERVERS.lock() {
            Ok(g) => g,
            Err(e) => {
                debug!("LSP: lock error in send_request: {e}");
                return;
            }
        };
        let server = match guard.get_mut(language) {
            Some(s) => s,
            None => {
                debug!("LSP: no server for language '{language}'");
                return;
            }
        };
        let id = server.next_id;
        server.next_id += 1;
        let request = parse::build_request(id, method, params);
        let msg = format!("Content-Length: {}\r\n\r\n{}", request.len(), request);
        {
            let pmap = crate::kernel::semantic::client::pending_map();
            if let Ok(mut pguard) = pmap.lock() {
                pguard.insert(id, method.to_string());
            }
        }
        (server.stdin.clone(), server.bg.clone(), msg, id, crate::kernel::semantic::client::pending_map())
    };

    bg.spawn(async move {
        let header = format!("Content-Length: {}\r\n\r\n", msg.len());
        let mut stdin = stdin_arc.lock().await;
        let _ = stdin.write_all(header.as_bytes()).await;
        let _ = stdin.write_all(msg.as_bytes()).await;
        let _ = stdin.flush().await;
    });
}

// ── C Functions ───────────────────────────────────────────────────────────

/// (lsp/start language-id command & args) → nil
unsafe extern "C-unwind" fn c_lsp_start(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let language = match conv::get_str(argc, argv, 0) {
            Some(l) => l,
            None => return conv::nil(),
        };
        let command = match conv::get_str(argc, argv, 1) {
            Some(c) => c,
            None => return conv::nil(),
        };
        let args: Vec<String> = (2..argc)
            .filter_map(|i| conv::get_str(argc, argv, i))
            .collect();

        if let Ok(guard) = SERVERS.lock()
            && guard.contains_key(&language) {
                return conv::nil();
            }

        let bg = match &ed.background {
            Some(bg) => bg.clone(),
            None => return conv::nil(),
        };

        let command_c = command.clone();
        let args_c = args.clone();
        let language_c = language.clone();
        let bg_clone = bg.clone();

        bg.spawn(async move {
            match LspServer::start(&command_c, &args_c, &language_c, bg_clone).await {
                Ok(server) => {
                    if let Ok(mut guard) = SERVERS.lock() {
                        guard.insert(language_c, server);
                    }
                }
                Err(e) => {
                    debug!("LSP: failed to start {command_c} for {language_c}: {e}");
                }
            }
        });

        conv::nil()
    })
}

/// (lsp/notify language-id method &opt json-params) → nil
unsafe extern "C-unwind" fn c_lsp_notify(argc: i32, argv: *mut Janet) -> Janet { unsafe {
    let language = match conv::get_str(argc, argv, 0) {
        Some(l) => l,
        None => return conv::nil(),
    };
    let method = match conv::get_str(argc, argv, 1) {
        Some(m) => m,
        None => return conv::nil(),
    };
    let params = conv::get_str(argc, argv, 2).unwrap_or_default();

    let msg = if params.is_empty() || params == "{}" {
        format!(r#"{{"jsonrpc":"2.0","method":"{}"}}"#, method)
    } else {
        format!(r#"{{"jsonrpc":"2.0","method":"{}","params":{}}}"#, method, params)
    };

    let (stdin_arc, bg) = {
        let guard = match SERVERS.lock() {
            Ok(g) => g,
            Err(_) => return conv::nil(),
        };
        let server = match guard.get(&language) {
            Some(s) => s,
            None => return conv::nil(),
        };
        (server.stdin.clone(), server.bg.clone())
    };

    bg.spawn(async move {
        let header = format!("Content-Length: {}\r\n\r\n", msg.len());
        let mut stdin = stdin_arc.lock().await;
        let _ = stdin.write_all(header.as_bytes()).await;
        let _ = stdin.write_all(msg.as_bytes()).await;
        let _ = stdin.flush().await;
    });

    conv::nil()
}}

/// (lsp/request language-id method params) → nil
/// Sends a generic LSP request. Response arrives via `lsp-response` event.
unsafe extern "C-unwind" fn c_lsp_request(argc: i32, argv: *mut Janet) -> Janet { unsafe {
    let language = match conv::get_str(argc, argv, 0) {
        Some(l) => l,
        None => return conv::nil(),
    };
    let method = match conv::get_str(argc, argv, 1) {
        Some(m) => m,
        None => return conv::nil(),
    };
    let params = match conv::get_str(argc, argv, 2) {
        Some(p) => p,
        None => "{}".to_string(),
    };
    send_request(&language, &method, &params);
    conv::nil()
}}

/// Helper: build a `file://` URI from the focused buffer's path.
fn focused_buffer_uri(ed: &crate::kernel::state::Editor) -> String {
    let buf_id = input::focused_buffer_id(ed);
    if let Some(buf) = ed.buffers.get(buf_id) {
        if let Some(ref path) = buf.path {
            format!("file://{}", path)
        } else {
            format!("untitled:{}", buf.name)
        }
    } else {
        String::new()
    }
}

/// (lsp/hover language-id) → nil
/// Request hover information at the cursor position in the focused buffer.
/// Response via `lsp-hover` event.
unsafe extern "C-unwind" fn c_lsp_hover(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let language = match conv::get_str(argc, argv, 0) {
            Some(l) => l,
            None => return conv::nil(),
        };
        let buf_id = input::focused_buffer_id(ed);
        let uri = focused_buffer_uri(ed);
        let (line, character) = if let Some(buf) = ed.buffers.get(buf_id) {
            let pos = buf.cursor;
            let text = buf.slice(0, pos);
            let line = text.chars().filter(|&c| c == '\n').count();
            let last_newline = text.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let character = text[last_newline..].chars().count();
            (line, character)
        } else {
            (0, 0)
        };
        let params = format!(
            r#"{{"textDocument":{{"uri":"{}"}},"position":{{"line":{},"character":{}}}}}"#,
            uri, line, character);
        send_request(&language, "textDocument/hover", &params);
        conv::nil()
    })
}

/// (lsp/code-actions language-id) → nil
/// Request code actions at the cursor position. Response via `lsp-code-actions` event.
unsafe extern "C-unwind" fn c_lsp_code_actions(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let language = match conv::get_str(argc, argv, 0) {
            Some(l) => l,
            None => return conv::nil(),
        };
        let uri = focused_buffer_uri(ed);
        let buf_id = input::focused_buffer_id(ed);
        let (line, character) = if let Some(buf) = ed.buffers.get(buf_id) {
            let pos = buf.cursor;
            let text = buf.slice(0, pos);
            let line = text.chars().filter(|&c| c == '\n').count();
            let last_newline = text.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let character = text[last_newline..].chars().count();
            (line, character)
        } else {
            (0, 0)
        };
        let params = format!(
            r#"{{"textDocument":{{"uri":"{}"}},"range":{{"start":{{"line":{},"character":{}}},"end":{{"line":{},"character":{}}}}},"context":{{"diagnostics":[]}}}}"#,
            uri, line, character, line, character + 1);
        send_request(&language, "textDocument/codeAction", &params);
        conv::nil()
    })
}

/// (lsp/completion language-id) → nil
/// Request completion items at the cursor position. Response via `lsp-completion-items` event.
unsafe extern "C-unwind" fn c_lsp_completion(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let language = match conv::get_str(argc, argv, 0) {
            Some(l) => l,
            None => return conv::nil(),
        };
        let uri = focused_buffer_uri(ed);
        let buf_id = input::focused_buffer_id(ed);
        let (line, character) = if let Some(buf) = ed.buffers.get(buf_id) {
            let pos = buf.cursor;
            let text = buf.slice(0, pos);
            let line = text.chars().filter(|&c| c == '\n').count();
            let last_newline = text.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let character = text[last_newline..].chars().count();
            (line, character)
        } else {
            (0, 0)
        };
        let params = format!(
            r#"{{"textDocument":{{"uri":"{}"}},"position":{{"line":{},"character":{}}}}}"#,
            uri, line, character);
        send_request(&language, "textDocument/completion", &params);
        conv::nil()
    })
}

/// (lsp/rename language-id new-name) → nil
/// Request rename of the symbol under the cursor. Response via `lsp-rename-result` event.
unsafe extern "C-unwind" fn c_lsp_rename(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let language = match conv::get_str(argc, argv, 0) {
            Some(l) => l,
            None => return conv::nil(),
        };
        let new_name = match conv::get_str(argc, argv, 1) {
            Some(n) => n,
            None => return conv::nil(),
        };
        let uri = focused_buffer_uri(ed);
        let buf_id = input::focused_buffer_id(ed);
        let (line, character) = if let Some(buf) = ed.buffers.get(buf_id) {
            let pos = buf.cursor;
            let text = buf.slice(0, pos);
            let line = text.chars().filter(|&c| c == '\n').count();
            let last_newline = text.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let character = text[last_newline..].chars().count();
            (line, character)
        } else {
            (0, 0)
        };
        let params = format!(
            r#"{{"textDocument":{{"uri":"{}"}},"position":{{"line":{},"character":{}}},"newName":"{}"}}"#,
            uri, line, character, new_name);
        send_request(&language, "textDocument/rename", &params);
        conv::nil()
    })
}

// ── Registration ────────────────────────────────────────────────────────────

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"lsp/start".as_ptr() as *const _,
            cfun: Some(c_lsp_start as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Start an LSP server for a language".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/notify".as_ptr() as *const _,
            cfun: Some(c_lsp_notify as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Send a notification to an LSP server".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/request".as_ptr() as *const _,
            cfun: Some(c_lsp_request as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Send a generic JSON-RPC request to an LSP server; response arrives via lsp-response event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/hover".as_ptr() as *const _,
            cfun: Some(c_lsp_hover as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Request hover information at a position; response via lsp-hover event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/code-actions".as_ptr() as *const _,
            cfun: Some(c_lsp_code_actions as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Request code actions at a position; response via lsp-code-actions event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/completion".as_ptr() as *const _,
            cfun: Some(c_lsp_completion as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Request completion items at a position; response via lsp-completion-items event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/rename".as_ptr() as *const _,
            cfun: Some(c_lsp_rename as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Request rename of symbol; response via lsp-rename-result event".as_ptr() as *const _,
        },
    ]
}
