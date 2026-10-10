//! Janet API for LSP — a thin surface over the kernel's `LspRegistry`.
//!
//! Every document request is made against the focused buffer's current
//! revision; the response event carries that context so stale results are
//! rejected on arrival (rework.md §2).  Document sync (`lsp/did-open`,
//! `lsp/did-change`, `lsp/did-close`) sends real document versions.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::semantic::lsp_proto;
use crate::kernel::state::Editor;

// ── C Functions ───────────────────────────────────────────────────────────

/// (lsp/start language-id command & args) → nil
unsafe extern "C-unwind" fn c_lsp_start(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let language = match conv::get_str(argc, argv, 0) {
            Some(l) => l.to_string(),
            None => return conv::nil(),
        };
        let command = match conv::get_str(argc, argv, 1) {
            Some(c) => c.to_string(),
            None => return conv::nil(),
        };
        let args: Vec<String> = (2..argc)
            .filter_map(|i| conv::get_str(argc, argv, i).map(String::from))
            .collect();

        if ed.lsp.get(&language).is_some() {
            return conv::nil();
        }
        let Some(bg) = ed.background.clone() else { return conv::nil() };
        let root = ed.project_manager.project.root.clone()
            .map(|p| p.to_string_lossy().into_owned());

        // The initialize handshake blocks up to 10s — it must never block
        // the foreground, so it runs on its own thread and the result lands
        // via the `LspStarted` background event.
        std::thread::Builder::new()
            .name(format!("lsp-init-{language}"))
            .spawn(move || {
                let result = crate::kernel::semantic::lsp::LspHandle::start(
                    &command, &args, &language, root, bg.clone()).map(std::sync::Arc::new);
                bg.send_event(crate::kernel::runtime::BackgroundEvent::LspStarted { language, result });
            })
            .map_err(|e| debug!("lsp-init thread: {e}"))
            .ok();
        conv::nil()
    })
}

/// (lsp/did-open buffer-id) → nil
/// Sends the full buffer text with its current revision as the version.
unsafe extern "C-unwind" fn c_lsp_did_open(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        with_server_and_doc(ed, argc, argv, |server, _buf_key, arc, uri| {
            let buf = arc.lock().unwrap();
            server.did_open(&uri, &buf.slice(0, buf.len()), buf.revision());
        });
        conv::nil()
    })
}

/// (lsp/did-change buffer-id) → nil
/// Full-text sync with the real document version; coalescing happens on the
/// caller side (the Janet plugin debounces buffer-changed).
unsafe extern "C-unwind" fn c_lsp_did_change(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        with_server_and_doc(ed, argc, argv, |server, _buf_key, arc, uri| {
            let buf = arc.lock().unwrap();
            server.did_change(&uri, &buf.slice(0, buf.len()), buf.revision());
        });
        conv::nil()
    })
}

/// (lsp/did-close buffer-id) → nil
unsafe extern "C-unwind" fn c_lsp_did_close(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        with_server_and_doc(ed, argc, argv, |server, _buf_key, _arc, uri| {
            server.did_close(&uri);
        });
        conv::nil()
    })
}

/// (lsp/notify language-id method &opt json-params) → nil
unsafe extern "C-unwind" fn c_lsp_notify(argc: i32, argv: *mut Janet) -> Janet { unsafe {
    let language = match conv::get_str(argc, argv, 0) {
        Some(l) => l.to_string(),
        None => return conv::nil(),
    };
    let method = match conv::get_str(argc, argv, 1) {
        Some(m) => m.to_string(),
        None => return conv::nil(),
    };
    let params: serde_json::Value = match conv::get_str(argc, argv, 2) {
        Some(p) if !p.is_empty() && p != "{}" =>
            serde_json::from_str(&p).unwrap_or(serde_json::json!({})),
        _ => serde_json::json!({}),
    };

    with_editor(|ed| {
        if let Some(server) = ed.lsp.get(&language) {
            server.notify(&method, params);
        }
        conv::nil()
    })
}}

/// (lsp/request language-id method params) → nil
/// Sends a generic request against the focused buffer's current revision.
/// The response arrives via the `lsp-response` event.
unsafe extern "C-unwind" fn c_lsp_request(argc: i32, argv: *mut Janet) -> Janet { unsafe {
    let language = match conv::get_str(argc, argv, 0) {
        Some(l) => l.to_string(),
        None => return conv::nil(),
    };
    let method = match conv::get_str(argc, argv, 1) {
        Some(m) => m.to_string(),
        None => return conv::nil(),
    };
    let params: serde_json::Value = match conv::get_str(argc, argv, 2) {
        Some(p) => serde_json::from_str(&p).unwrap_or(serde_json::json!({})),
        None => serde_json::json!({}),
    };
    with_editor(|ed| {
        if let Some(server) = ed.lsp.get(&language) {
            let (buf_key, revision) = focused_revision(ed);
            server.request(&method, params, buf_key, revision);
        }
        conv::nil()
    })
}}

/// Focused buffer key and its current revision — the request context every
/// document-scoped LSP request carries.
fn focused_revision(ed: &Editor) -> (usize, u64) {
    let buf_key = crate::kernel::input::focused_buffer_id(ed);
    let revision = ed.buffers.get(buf_key)
        .map(|a| a.lock().unwrap().revision())
        .unwrap_or(0);
    (buf_key, revision)
}

/// Run `f` with the server named by argv[0], the buffer named by argv[1],
/// its text Arc, and its file URI.  The language mapping lives in the Janet
/// plugin (the extension→server table); the kernel only resolves servers.
unsafe fn with_server_and_doc(
    ed: &mut Editor,
    argc: i32,
    argv: *mut Janet,
    f: impl FnOnce(&std::sync::Arc<crate::kernel::semantic::lsp::LspHandle>, usize, &std::sync::Arc<std::sync::Mutex<crate::kernel::text_engine::Buffer>>, String),
) {
    unsafe {
        let Some(language) = conv::get_str(argc, argv, 0).map(String::from) else { return };
        let buf_key = if argc > 1 {
            conv::get_int(argc, argv, 1).unwrap_or(-1).max(0) as usize
        } else {
            crate::kernel::input::focused_buffer_id(ed)
        };
        let Some(server) = ed.lsp.get(&language).cloned() else { return };
        let Some(arc) = ed.buffers.get(buf_key).cloned() else { return };
        let uri = {
            let buf = arc.lock().unwrap();
            match buf.path.as_deref() {
                Some(p) => lsp_proto::path_to_uri(p),
                None => format!("untitled:{}", buf.name),
            }
        };
        f(&server, buf_key, &arc, uri);
    }
}

/// (lsp/hover language-id) → nil
unsafe extern "C-unwind" fn c_lsp_hover(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        request_at_cursor(ed, argc, argv, "textDocument/hover", |line, character| {
            serde_json::json!({
                "textDocument": { "uri": "" },
                "position": { "line": line, "character": character },
            })
        });
        conv::nil()
    })
}

/// (lsp/code-actions language-id) → nil
unsafe extern "C-unwind" fn c_lsp_code_actions(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        request_at_cursor(ed, argc, argv, "textDocument/codeAction", |line, character| {
            serde_json::json!({
                "textDocument": { "uri": "" },
                "range": {
                    "start": { "line": line, "character": character },
                    "end": { "line": line, "character": character + 1 },
                },
                "context": { "diagnostics": [] },
            })
        });
        conv::nil()
    })
}

/// (lsp/completion language-id) → nil
unsafe extern "C-unwind" fn c_lsp_completion(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        request_at_cursor(ed, argc, argv, "textDocument/completion", |line, character| {
            serde_json::json!({
                "textDocument": { "uri": "" },
                "position": { "line": line, "character": character },
            })
        });
        conv::nil()
    })
}

/// (lsp/rename language-id new-name) → nil
unsafe extern "C-unwind" fn c_lsp_rename(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let new_name = match conv::get_str(argc, argv, 1) {
            Some(n) => n.to_string(),
            None => return conv::nil(),
        };
        request_at_cursor(ed, argc, argv, "textDocument/rename", |line, character| {
            serde_json::json!({
                "textDocument": { "uri": "" },
                "position": { "line": line, "character": character },
                "newName": new_name,
            })
        });
        conv::nil()
    })
}

/// Send a cursor-scoped request for the language in argv[0], filling in the
/// focused buffer's URI and UTF-16 position.
unsafe fn request_at_cursor(
    ed: &mut Editor,
    argc: i32,
    argv: *mut Janet,
    method: &str,
    params: impl FnOnce(usize, usize) -> serde_json::Value,
) {
    unsafe {
        let Some(language) = conv::get_str(argc, argv, 0).map(String::from) else { return };
        let Some(server) = ed.lsp.get(&language).cloned() else { return };
        let buf_key = crate::kernel::input::focused_buffer_id(ed);
        let Some(arc) = ed.buffers.get(buf_key).cloned() else { return };
        let (line, character, uri) = {
            let buf = arc.lock().unwrap();
            let cursor = ed.view_of_buffer(buf_key).map(|v| v.cursor_offset()).unwrap_or(0);
            let (l, _col_bytes) = buf.line_col_bytes(cursor);
            let (_l, character) = buf.lsp_position(cursor);
            let uri = match buf.path.as_deref() {
                Some(p) => lsp_proto::path_to_uri(p),
                None => format!("untitled:{}", buf.name),
            };
            (l, character, uri)
        };
        let (buf_key, revision) = focused_revision(ed);
        let mut params = params(line, character);
        if let Some(obj) = params.as_object_mut()
            && let Some(doc) = obj.get_mut("textDocument") {
                if let Some(uri_field) = doc.get_mut("uri") {
                    *uri_field = serde_json::Value::String(uri);
                }
            }
        server.request(method, params, buf_key, revision);
    }
}

/// (lsp/shutdown language-id) → nil
unsafe extern "C-unwind" fn c_lsp_shutdown(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if let Some(language) = conv::get_str(argc, argv, 0).map(String::from) {
            if let Some(server) = ed.lsp.get(&language) {
                server.shutdown();
            }
            ed.lsp.exited(&language);
        }
        conv::nil()
    })
}

// ── Registration ────────────────────────────────────────────────────────────

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"lsp/start".as_ptr() as *const _,
            cfun: Some(c_lsp_start as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Start an LSP server for a language (initialize is awaited)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/did-open".as_ptr() as *const _,
            cfun: Some(c_lsp_did_open as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Send textDocument/didOpen for a buffer with its current revision as version".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/did-change".as_ptr() as *const _,
            cfun: Some(c_lsp_did_change as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Send textDocument/didChange (full text, real version) for a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/did-close".as_ptr() as *const _,
            cfun: Some(c_lsp_did_close as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Send textDocument/didClose for a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/notify".as_ptr() as *const _,
            cfun: Some(c_lsp_notify as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Send a notification to an LSP server".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/request".as_ptr() as *const _,
            cfun: Some(c_lsp_request as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Send a generic JSON-RPC request; response arrives via lsp-response event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/hover".as_ptr() as *const _,
            cfun: Some(c_lsp_hover as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Request hover information at the cursor; response via lsp-hover event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/code-actions".as_ptr() as *const _,
            cfun: Some(c_lsp_code_actions as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Request code actions at the cursor; response via lsp-code-actions event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/completion".as_ptr() as *const _,
            cfun: Some(c_lsp_completion as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Request completion at the cursor; response via lsp-completion-items event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/rename".as_ptr() as *const _,
            cfun: Some(c_lsp_rename as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Request rename of the symbol at the cursor; response via lsp-rename-result event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"lsp/shutdown".as_ptr() as *const _,
            cfun: Some(c_lsp_shutdown as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Shut down a language server (ordered shutdown + exit)".as_ptr() as *const _,
        },
    ]
}
