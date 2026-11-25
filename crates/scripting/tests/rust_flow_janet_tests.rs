//! End-to-end flow for the real bundled rust plugin: open a .rs file,
//! type new code, and watch the tree-sitter lane re-highlight it; attach
//! a language server and watch LSP completions (with a snippet) land in
//! the popup.  These are the two user-visible paths that were dead when
//! modes and payload keys regressed.

#[macro_use]
mod helpers;
use scripting;

fn workspace_plugins() -> String {
    // crates/scripting/tests → workspace root holds the bundled plugins.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins")
        .canonicalize()
        .unwrap();
    dir.to_str().unwrap().replace('\\', "/")
}

fn rs_file(tag: &str, body: &str) -> String {
    let dir = std::env::temp_dir().join(format!("magma_rust_flow_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("main.rs");
    std::fs::write(&file, body).unwrap();
    // The attach layer starts marker-policy servers only inside a real
    // project — the same behavior a user gets opening a file from a repo.
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"t\"\n").unwrap();
    file.to_str().unwrap().replace('\\', "/")
}

fn open_file_cmd(ed: &mut kernel::state::Editor, path: &str) {
    let mut args = std::collections::HashMap::new();
    args.insert(
        "path".to_string(),
        kernel::command::args::ArgValue::Path(path.to_string()),
    );
    kernel::command::execute_command(ed, "open-file", &args).unwrap();
}

fn enable_rust_plugin() -> String {
    format!(
        r#"(plugin/discover "{}") (plugin/enable "rust")"#,
        workspace_plugins()
    )
}

fn syntax_layer_len(ed: &kernel::state::Editor, key: usize) -> usize {
    ed.views
        .get(&key)
        .and_then(|v| v.highlight_layers.get("syntax"))
        .map(|r| r.len())
        .unwrap_or(0)
}

fn layer_covers(
    ed: &kernel::state::Editor,
    key: usize,
    start_byte: usize,
    end_byte: usize,
) -> bool {
    ed.views
        .get(&key)
        .and_then(|v| v.highlight_layers.get("syntax"))
        .map(|ranges| {
            ranges
                .iter()
                .any(|(s, e, _)| *s < end_byte && start_byte < *e)
        })
        .unwrap_or(false)
}

#[test]
fn typing_new_code_gets_highlighted_through_the_parse_lane() {
    let path = rs_file(
        "hl",
        "fn main() {\n    let alpha = 1;\n    println!(\"{}\", alpha);\n}\n",
    );
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    // A real background pool, like the shell boots: the parse lane runs there.
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(kernel::runtime::BackgroundHandle::new(runtime, tx));

    assert_eq!(
        scripting::eval(&enable_rust_plugin()),
        "ok",
        "the real rust plugin must load (grammar + query)"
    );
    open_file_cmd(&mut ed, &path);
    ed.events.drain_and_dispatch();

    let key = helpers::focused_key(&ed);
    assert_eq!(
        ed.buffers
            .get(key)
            .unwrap()
            .lock()
            .unwrap()
            .major_mode
            .name(),
        "rust-mode"
    );
    assert_eq!(
        ed.ts_languages.get(&key).map(String::as_str),
        Some("rust"),
        "the plugin grammar must be attached to the buffer"
    );
    assert!(
        syntax_layer_len(&ed, key) > 0,
        "existing code is highlighted at open time"
    );
    {
        // The shipped query must paint keywords — the fallback's few dim
        // keyword spans were the only color before, so this is the
        // regression guard for "syntax highlighting looks plain".
        let layer = ed
            .views
            .get(&key)
            .unwrap()
            .highlight_layers
            .get("syntax")
            .unwrap();
        assert!(
            layer
                .iter()
                .any(|(_, _, scope)| scope.starts_with("keyword")),
            "keywords must carry the keyword scope, got {layer:?}"
        );
        assert!(
            layer
                .iter()
                .any(|(_, _, scope)| scope.starts_with("string")),
            "strings must carry the string scope, got {layer:?}"
        );
    }

    // "Type" new code: a real transaction through the publication path.
    let len = {
        let arc = ed.buffers.get(key).unwrap();
        let mut buf = arc.lock().unwrap();
        let l = buf.len();
        buf.insert(l, "    let beta_gamma = alpha + 1;\n");
        l
    };
    ed.dispatch_events_scoped();

    // The lane parses off-thread and reports through TsTreeReady.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut got_tree = false;
    while std::time::Instant::now() < deadline {
        match rx.try_recv() {
            Ok(ev) => {
                if matches!(ev, kernel::runtime::BackgroundEvent::TsTreeReady { .. }) {
                    kernel::runtime::process_background_event(&mut ed, ev);
                    got_tree = true;
                    break;
                }
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
    assert!(got_tree, "the parse lane must deliver a fresh tree");
    ed.dispatch_events_scoped();

    {
        let view = ed.views.get(&key).unwrap();
        eprintln!(
            "DBG final layer: {:?} insert_at={}",
            view.highlight_layers.get("syntax"),
            len
        );
    }
    let new_end = len + "    let beta_gamma = alpha + 1;\n".len();
    assert!(
        layer_covers(&ed, key, len, new_end),
        "newly typed code must carry syntax spans after the lane lands"
    );
}

// ── Comments ─────────────────────────────────────────────────────────────

/// The face a scope resolves to, the way the renderer resolves it: the
/// longest matching scope-face prefix wins, then a face of the same name.
fn face_for_scope(ed: &kernel::state::Editor, scope: &str) -> Option<String> {
    ed.scope_faces
        .iter()
        .find(|(prefix, _)| scope.starts_with(prefix.as_str()))
        .map(|(_, face)| face.clone())
        .or_else(|| ed.faces.contains_key(scope).then(|| scope.to_string()))
}

/// Every comment flavor the grammar distinguishes must color as a comment.
/// A doc comment is a `line_comment`/`block_comment` node carrying marker +
/// doc children — the query's documentation capture has to resolve to the
/// same face as `//`, not to the generic fallback the renderer uses for
/// scopes it cannot resolve.
#[test]
fn doc_comments_color_like_plain_comments_through_the_lane() {
    let body = "\
// plain comment
/// Outer doc comment
//! Inner doc comment
//// four slashes stay a plain comment
/** Block doc comment */
/*! Inner block doc comment */
/* plain block comment */
fn main() {}
";
    let path = rs_file("comments", body);
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    assert_eq!(scripting::eval(&enable_rust_plugin()), "ok");
    open_file_cmd(&mut ed, &path);
    ed.events.drain_and_dispatch();

    let key = helpers::focused_key(&ed);
    let layer = ed
        .views
        .get(&key)
        .unwrap()
        .highlight_layers
        .get("syntax")
        .cloned()
        .unwrap_or_default();
    assert!(!layer.is_empty(), "the open path must highlight the file");

    let mut offset = 0;
    let mut checked = 0;
    for line in body.lines() {
        if line.starts_with("//") || line.starts_with("/*") {
            let scope = layer
                .iter()
                .find(|(start, end, _)| *start <= offset && offset + line.len() <= *end)
                .map(|(_, _, scope)| scope.clone())
                .unwrap_or_else(|| panic!("{line:?} must carry a highlight span, got {layer:?}"));
            let face = face_for_scope(&ed, &scope)
                .unwrap_or_else(|| panic!("{line:?}: scope {scope:?} resolves to no face"));
            assert_eq!(
                face, "comment-face",
                "{line:?} must color as a comment, got scope {scope:?}"
            );
            checked += 1;
        }
        offset += line.len() + 1;
    }
    assert_eq!(checked, 7, "every comment flavor must be checked");
}

/// Deleting a line break leaves the cached tree one edit behind the buffer.
/// That edit's old end point cannot be read off the post-edit text, so the
/// highlight path must reparse whole — the layer it paints has to be
/// exactly what a from-scratch parse produces.
#[test]
fn deleting_a_newline_keeps_the_layer_equal_to_a_full_parse() {
    let body = "\
//! Module docs.
use std::fmt;

/// Joins the two lines below when the break at the end
/// of this line is deleted.
fn main() {
    let value = format!(\"{}\", 1);
    println!(\"{value}\");
}
";
    let path = rs_file("newline_edit", body);
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    assert_eq!(scripting::eval(&enable_rust_plugin()), "ok");
    open_file_cmd(&mut ed, &path);
    ed.events.drain_and_dispatch();

    let key = helpers::focused_key(&ed);
    let syntax_layer = |ed: &kernel::state::Editor| -> Vec<(usize, usize, String)> {
        ed.views
            .get(&key)
            .unwrap()
            .highlight_layers
            .get("syntax")
            .cloned()
            .unwrap_or_default()
    };
    assert!(!syntax_layer(&ed).is_empty(), "open must highlight the file");

    // Delete the line break inside the doc comment — the tree in the cache
    // is now one edit behind the text.
    let break_at = body.find("of this line").unwrap();
    let newline = body[..break_at].rfind('\n').unwrap();
    {
        let arc = ed.buffers.get(key).unwrap();
        let mut buf = arc.lock().unwrap();
        buf.delete(newline, newline + 1);
    }
    assert_eq!(
        scripting::eval(&format!(r#"(do (ts/highlight-buffer {key}) "ok")"#)),
        "ok"
    );
    let after_edit = syntax_layer(&ed);

    // The same text highlighted with no cached tree: the from-scratch path.
    ed.ts_trees.remove(&key);
    assert_eq!(
        scripting::eval(&format!(r#"(do (ts/highlight-buffer {key}) "ok")"#)),
        "ok"
    );
    let from_scratch = syntax_layer(&ed);

    assert_eq!(
        after_edit, from_scratch,
        "highlighting from a one-edit-stale tree must match a full parse"
    );
}

// ── LSP completions ──────────────────────────────────────────────────────

#[cfg(unix)]
const FAKE_RA: &str = r#"#!/usr/bin/env python3
import sys, json

def read_msg():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            raise EOFError("stdin closed")
        if line in (b"\r\n", b"\n"):
            break
        text = line.decode("utf-8", "replace")
        key, _, value = text.partition(":")
        headers[key.strip().lower()] = value.strip()
    n = int(headers["content-length"])
    return json.loads(sys.stdin.buffer.read(n))

def write_msg(obj):
    body = json.dumps(obj).encode()
    sys.stdout.buffer.write(b"Content-Length: %d\r\n\r\n" % len(body))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()

while True:
    try:
        msg = read_msg()
    except Exception:
        break
    method = msg.get("method")
    if method == "initialize":
        write_msg({"jsonrpc": "2.0", "id": msg["id"], "result": {"capabilities": {}}})
    elif method == "textDocument/completion":
        write_msg({"jsonrpc": "2.0", "id": msg["id"], "result": {"items": [
            {"label": "alpine_fetch", "kind": 3, "detail": "fn fetch(&self)",
             "insertTextFormat": 2, "insertText": "alpine_fetch($1){}"}
        ]}})
    elif "id" in msg:
        write_msg({"jsonrpc": "2.0", "id": msg["id"], "result": None})
"#;

#[cfg(unix)]
#[test]
fn lsp_completions_reach_the_popup_with_snippets() {
    let dir = std::env::temp_dir().join(format!("magma_fake_ra_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake-ra");
    std::fs::write(&script, FAKE_RA).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let path = rs_file("lsp", "fn main() {\n    let alp = 1;\n}\n");
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    // The spawn and the response channel both need the background pool.
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(kernel::runtime::BackgroundHandle::new(runtime, tx));

    // The attach resolver searches $PATH — put the fake server there.
    // Test-process-local: the attach resolver reads PATH at call time.
    let old_path = std::env::var("PATH").unwrap_or_default();
    unsafe { std::env::set_var("PATH", format!("{}:{old_path}", dir.to_str().unwrap())) };

    assert_eq!(scripting::eval(&enable_rust_plugin()), "ok");
    // The plugin config set the real server; point the override at the fake.
    assert_eq!(
        scripting::eval(r#"(option/set "rust-lsp-command" "fake-ra")"#),
        "ok"
    );
    open_file_cmd(&mut ed, &path);
    ed.events.drain_and_dispatch();

    // The handshake runs on its own thread; its result is a background
    // event.  Pump the channel until the attach layer flips to ready.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut ready = false;
    while std::time::Instant::now() < deadline {
        while let Ok(ev) = rx.try_recv() {
            kernel::runtime::process_background_event(&mut ed, ev.clone());
            if matches!(ev, kernel::runtime::BackgroundEvent::Lsp(_)) {
                if let kernel::runtime::BackgroundEvent::Lsp(kernel::semantic::Event::Started {
                    ..
                }) = ev
                {
                    ed.events.drain_and_dispatch();
                }
            }
        }
        ready = scripting::eval_result(r#"(string (= :ready (get *lsp-servers* "rust")))"#)
            .unwrap_or_default()
            == r#""true""#;
        if ready {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    unsafe { std::env::set_var("PATH", old_path) };
    assert!(ready, "the fake language server must reach :ready");

    // Cursor after "alp", then trigger — the popup fills with words while
    // the request is in flight; the server item leads once it lands.
    let key = helpers::focused_key(&ed);
    {
        // "fn main() {\n    let alp" — right after the partial word.
        let view = ed.views.get_mut(&key).unwrap();
        view.set_cursor(23);
    }
    let _ = scripting::eval(r#"(editor/run-command "completion-trigger")"#);
    ed.events.drain_and_dispatch();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        let mut drained = false;
        while let Ok(ev) = rx.try_recv() {
            kernel::runtime::process_background_event(&mut ed, ev);
            drained = true;
        }
        if drained {
            ed.events.drain_and_dispatch();
        }
        if ed
            .completion
            .items
            .iter()
            .any(|i| i.label == "alpine_fetch")
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let item = ed
        .completion
        .items
        .iter()
        .find(|i| i.label == "alpine_fetch")
        .expect("the server completion must reach the popup");
    assert!(item.is_snippet, "the LSP item carries snippet syntax");
    assert_eq!(item.insert, "alpine_fetch($1){}");
    assert_eq!(item.detail, "fn fetch(&self)");
}

// ── LSP inlay hints ──────────────────────────────────────────────────────
//
// The request fires on publishDiagnostics, not on didOpen, so a fake
// server must publish (as real ones do) before any hint can be asked for.

#[cfg(unix)]
const FAKE_RA_HINTS: &str = r#"#!/usr/bin/env python3
import sys, json

def read_msg():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            raise EOFError("stdin closed")
        if line in (b"\r\n", b"\n"):
            break
        text = line.decode("utf-8", "replace")
        key, _, value = text.partition(":")
        headers[key.strip().lower()] = value.strip()
    n = int(headers["content-length"])
    return json.loads(sys.stdin.buffer.read(n))

def write_msg(obj):
    body = json.dumps(obj).encode()
    sys.stdout.buffer.write(b"Content-Length: %d\r\n\r\n" % len(body))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()

while True:
    try:
        msg = read_msg()
    except Exception:
        break
    method = msg.get("method")
    if method == "initialize":
        write_msg({"jsonrpc": "2.0", "id": msg["id"], "result": {"capabilities": {}}})
    elif method == "textDocument/didOpen":
        td = msg["params"]["textDocument"]
        write_msg({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
                   "params": {"uri": td["uri"], "version": td["version"],
                              "diagnostics": []}})
    elif method == "textDocument/inlayHint":
        write_msg({"jsonrpc": "2.0", "id": msg["id"], "result": [
            {"position": {"line": 1, "character": 13}, "label": ": i32",
             "paddingRight": True}
        ]})
    elif "id" in msg:
        write_msg({"jsonrpc": "2.0", "id": msg["id"], "result": None})
"#;

#[cfg(unix)]
#[test]
fn lsp_inlay_hints_land_in_the_view_after_diagnostics() {
    let dir = std::env::temp_dir().join(format!("magma_fake_ra_hints_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake-ra");
    std::fs::write(&script, FAKE_RA_HINTS).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let path = rs_file("hints", "fn main() {\n    let alpha = 1;\n}\n");
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(kernel::runtime::BackgroundHandle::new(runtime, tx));

    let old_path = std::env::var("PATH").unwrap_or_default();
    unsafe { std::env::set_var("PATH", format!("{}:{old_path}", dir.to_str().unwrap())) };

    assert_eq!(scripting::eval(&enable_rust_plugin()), "ok");
    assert_eq!(
        scripting::eval(r#"(option/set "rust-lsp-command" "fake-ra")"#),
        "ok"
    );
    open_file_cmd(&mut ed, &path);
    ed.events.drain_and_dispatch();

    // Pump until the server is ready, then until the hint decoration
    // lands — the diagnostics publish is what triggers the request.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut requests_seen = 0usize;
    while std::time::Instant::now() < deadline {
        while let Ok(ev) = rx.try_recv() {
            if let kernel::runtime::BackgroundEvent::Lsp(
                kernel::semantic::Event::Started { .. },
            ) = &ev
            {
                kernel::runtime::process_background_event(&mut ed, ev);
                ed.events.drain_and_dispatch();
            } else {
                kernel::runtime::process_background_event(&mut ed, ev);
            }
        }
        let key = helpers::focused_key(&ed);
        if ed
            .view_of_buffer(key)
            .map(|v| v.decor_count_layer("lsp-inlay"))
            .unwrap_or(0)
            > 0
        {
            break;
        }
        if requests_seen == 0
            && scripting::eval_result(r#"(string (= :ready (get *lsp-servers* "rust")))"#)
                .unwrap_or_default()
                == r#""true""#
        {
            requests_seen += 1;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    unsafe { std::env::set_var("PATH", old_path) };

    let key = helpers::focused_key(&ed);
    assert!(
        scripting::eval_result(r#"(string (= :ready (get *lsp-servers* "rust")))"#)
            .unwrap_or_default()
            == r#""true""#,
        "the fake server must reach :ready"
    );
    let count = ed
        .view_of_buffer(key)
        .map(|v| v.decor_count_layer("lsp-inlay"))
        .unwrap_or(0);
    assert!(count > 0, "an inlay hint must decorate the buffer view");
    let view = ed.view_of_buffer(key).unwrap();
    let decors = view.decoration_layers.get("lsp-inlay").unwrap();
    assert!(
        decors.iter().any(
            |d| matches!(d, kernel::text_engine::Decoration::InlineText { line: 1, text, .. }
                if text == ": i32 ")
        ),
        "the hint label with its right padding must be present: {decors:?}"
    );
}

// ── Real rust-analyzer, real project ─────────────────────────────────────
//
// The fake-server test above proves the plumbing; this one proves the
// real server attaches through the live attach path (project-opened →
// lsp/start with a rootUri, real handshake, real indexing) and serves
// semantic completions.  Skips silently when rust-analyzer is absent
// (fresh CI machines).

#[test]
fn real_rust_analyzer_serves_semantic_completions() {
    // Spawns a live rust-analyzer and waits out its first analysis —
    // minutes on a cold cache.  Opt-in so `cargo test` stays fast:
    // MAGMA_RA_E2E=1 cargo test -p scripting --test rust_flow_janet_tests
    if std::env::var("MAGMA_RA_E2E").as_deref() != Ok("1") {
        return;
    }
    let Some(_ra) = kernel::project::toolchain::find("rust-analyzer") else {
        eprintln!("rust-analyzer not installed; skipping");
        return;
    };

    let dir = std::env::temp_dir().join(format!("magma_ra_live_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"ra_live\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    let src = dir.join("src/main.rs");
    std::fs::write(&src, "fn main() {\n    let s = Str\n}\n").unwrap();

    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(kernel::runtime::BackgroundHandle::new(runtime, tx));

    assert_eq!(scripting::eval(&enable_rust_plugin()), "ok");

    // Live boot order: project first (the plugin starts the server on
    // project-opened, with the root), then the file.
    let mut args = std::collections::HashMap::new();
    args.insert(
        "path".to_string(),
        kernel::command::args::ArgValue::Path(dir.to_string_lossy().into_owned()),
    );
    kernel::command::execute_command(&mut ed, "open-project", &args).unwrap();
    let path = src.to_str().unwrap().replace('\\', "/");
    open_file_cmd(&mut ed, &path);
    ed.events.drain_and_dispatch();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut ready = false;
    while std::time::Instant::now() < deadline {
        while let Ok(ev) = rx.try_recv() {
            let is_started = matches!(
                &ev,
                kernel::runtime::BackgroundEvent::Lsp(kernel::semantic::Event::Started { .. })
            );
            kernel::runtime::process_background_event(&mut ed, ev);
            if is_started {
                ed.events.drain_and_dispatch();
            }
        }
        ready = scripting::eval_result(r#"(string (= :ready (get *lsp-servers* "rust")))"#)
            .unwrap_or_default()
            == r#""true""#;
        if ready {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(
        ready,
        "rust-analyzer must reach :ready against a real project"
    );

    // Cursor after "Str", trigger, and wait out rust-analyzer's first
    // analysis — semantic items must lead the popup.
    let key = helpers::focused_key(&ed);
    {
        let view = ed.views.get_mut(&key).unwrap();
        view.set_cursor(24); // "fn main() {\n    let s = Str"
    }
    eprintln!(
        "DBG lang_for_buffer={:?} server_running={:?}",
        ed.lsp.language_for_buffer(key),
        ed.lsp.running()
    );
    {
        let buf = ed.buffers.get(key).unwrap().lock().unwrap();
        eprintln!(
            "DBG buf_len={} cursor={:?} text={:?}",
            buf.len(),
            ed.view_of_buffer(key).map(|v| v.cursor.offset),
            buf.slice(0, buf.len())
        );
    }
    eprintln!(
        "DBG lsp_pos24={:?}",
        scripting::eval_result(&format!(
            r#"(string/format "%j" (buffer/lsp-position {key} 24))"#
        ))
    );
    eprintln!(
        "DBG lsp_pos24_rust={:?} rev={}",
        ed.buffers
            .get(key)
            .unwrap()
            .lock()
            .unwrap()
            .lsp_position(24),
        ed.buffers.get(key).unwrap().lock().unwrap().revision()
    );
    let _ = scripting::eval(r#"(editor/run-command "completion-trigger")"#);
    ed.events.drain_and_dispatch();
    eprintln!(
        "DBG popup_after_trigger={:?}",
        ed.completion
            .items
            .iter()
            .map(|i| i.label.clone())
            .collect::<Vec<_>>()
    );

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut semantic = false;
    while std::time::Instant::now() < deadline {
        while let Ok(ev) = rx.try_recv() {
            if let kernel::runtime::BackgroundEvent::Lsp(e) = &ev {
                eprintln!("DBG lsp_event: {e:?}");
            }
            kernel::runtime::process_background_event(&mut ed, ev);
        }
        ed.events.drain_and_dispatch();
        semantic = ed.completion.items.iter().any(|i| !i.kind.is_empty());
        if semantic {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    eprintln!(
        "DBG final_popup={:?}",
        ed.completion
            .items
            .iter()
            .map(|i| (i.label.clone(), i.kind.clone()))
            .collect::<Vec<_>>()
    );
    assert!(
        semantic,
        "rust-analyzer completions (with kinds) must reach the popup; got {:?}",
        ed.completion
            .items
            .iter()
            .map(|i| (i.label.clone(), i.kind.clone()))
            .collect::<Vec<_>>()
    );
    // No orphans: the spawned server must not outlive the test.
    ed.lsp.shutdown_all();
}

#[test]
fn opening_a_file_publishes_fold_candidates_without_typing() {
    // The regression for "folds don't work": a fresh open must arrive with
    // fold candidates already published — the first tree lands on
    // buffer-created, and the recompute runs on the focus event open-file
    // now emits.  Typing must not be required.
    let path = rs_file(
        "fold",
        "fn main() {\n    let alpha = 1;\n    println!(\"{}\", alpha);\n}\n\nfn second() {\n    let beta = 2;\n}\n",
    );
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    assert_eq!(
        scripting::eval(&enable_rust_plugin()),
        "ok",
        "the real rust plugin must load (grammar + queries)"
    );
    open_file_cmd(&mut ed, &path);
    ed.events.drain_and_dispatch();

    let key = helpers::focused_key(&ed);
    let ranges = ed.views.get(&key).unwrap().fold_ranges.clone();
    assert!(
        !ranges.is_empty(),
        "a freshly opened file must have fold candidates"
    );

    // The cursor sits at offset 0 — on `fn main`'s header line — so a
    // toggle folds that function, and a second toggle opens it again.
    assert_eq!(scripting::eval("(command/run \"toggle-fold\")"), "ok");
    let folds = ed.views.get(&key).unwrap().folds.clone();
    assert_eq!(folds.len(), 1, "toggle-fold collapsed the first function");
    assert_eq!(folds[0].0, 0, "the fold starts on the header line");
    assert_eq!(scripting::eval("(command/run \"toggle-fold\")"), "ok");
    assert!(
        ed.views.get(&key).unwrap().folds.is_empty(),
        "a second toggle opens the fold again"
    );
}

#[test]
fn real_rust_analyzer_serves_semantic_tokens() {
    // The regression this guards: the client's initialize capabilities
    // once missed the semanticTokens legend arrays rust-analyzer
    // deserializes strictly — every server spawn aborted mid-handshake,
    // so no LSP feature worked at all.  A ready server plus a populated
    // "semantic" layer proves the whole path.
    //
    // Spawns a live rust-analyzer; opt-in so `cargo test` stays fast:
    // MAGMA_RA_E2E=1 cargo test -p scripting --test rust_flow_janet_tests
    if std::env::var("MAGMA_RA_E2E").as_deref() != Ok("1") {
        return;
    }
    let Some(_ra) = kernel::project::toolchain::find("rust-analyzer") else {
        eprintln!("rust-analyzer not installed; skipping");
        return;
    };

    let dir = std::env::temp_dir().join(format!("magma_ra_semtok_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"ra_semtok\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    let src = dir.join("src/main.rs");
    std::fs::write(
        &src,
        "fn main() {\n    let unused_var = 1;\n    println!(\"hi\");\n}\n",
    )
    .unwrap();

    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(kernel::runtime::BackgroundHandle::new(runtime, tx));

    assert_eq!(scripting::eval(&enable_rust_plugin()), "ok");

    // Live boot order: project first (the plugin starts the server on
    // project-opened, with the root), then the file.
    let mut args = std::collections::HashMap::new();
    args.insert(
        "path".to_string(),
        kernel::command::args::ArgValue::Path(dir.to_string_lossy().into_owned()),
    );
    kernel::command::execute_command(&mut ed, "open-project", &args).unwrap();
    let path = src.to_str().unwrap().replace('\\', "/");
    open_file_cmd(&mut ed, &path);
    ed.events.drain_and_dispatch();

    let key = helpers::focused_key(&ed);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    let mut ready = false;
    let mut tokens = false;
    while std::time::Instant::now() < deadline {
        while let Ok(ev) = rx.try_recv() {
            let started = matches!(
                &ev,
                kernel::runtime::BackgroundEvent::Lsp(kernel::semantic::Event::Started { .. })
            );
            kernel::runtime::process_background_event(&mut ed, ev);
            if started {
                ed.events.drain_and_dispatch();
            }
        }
        ed.events.drain_and_dispatch();
        ready = scripting::eval_result(r#"(string (= :ready (get *lsp-servers* "rust")))"#)
            .unwrap_or_default()
            == r#""true""#;
        tokens = ed
            .views
            .get(&key)
            .and_then(|v| v.highlight_layers.get("semantic"))
            .map(|l| !l.is_empty())
            .unwrap_or(false);
        if ready && tokens {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(
        ready,
        "rust-analyzer must reach :ready — a handshake abort means the initialize capabilities were rejected"
    );
    assert!(
        tokens,
        "semantic tokens must reach the semantic highlight layer"
    );

    let scopes: Vec<String> = ed
        .views
        .get(&key)
        .unwrap()
        .highlight_layers
        .get("semantic")
        .unwrap()
        .iter()
        .map(|(_, _, s)| s.clone())
        .collect();
    assert!(
        scopes.iter().any(|s| s == "function" || s == "keyword"),
        "server token types must survive the resolvable-scope filter, got {scopes:?}"
    );

    // Positions, not just presence: token deltas are relative, and a decoder
    // that treats them as absolute paints every token after the first at a
    // shifted offset — garbage that lands ON TOP of the syntax layer.  The
    // file starts "fn main() {…", so the `main` token must be exactly bytes
    // 3..7, and no token can straddle the newline at byte 12.
    let layer = ed
        .views
        .get(&key)
        .unwrap()
        .highlight_layers
        .get("semantic")
        .unwrap()
        .clone();
    assert!(
        layer.iter().any(|&(s, e, _)| (s, e) == (3, 7)),
        "the `main` token must decode at bytes 3..7, spans: {layer:?}"
    );
    let text = ed
        .buffers
        .get(key)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).slice(0, 64).to_string())
        .unwrap_or_default();
    for (s, e, _) in &layer {
        let covered = text
            .get(*s..(*e).min(text.len()))
            .map(|x| x.contains('\n'))
            .unwrap_or(true);
        assert!(
            !covered,
            "no server token spans a line break; [{s}..{e}) does — deltas were mis-decoded"
        );
    }
    // No orphans: the spawned server must not outlive the test.
    ed.lsp.shutdown_all();
}
