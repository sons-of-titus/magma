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

// ── LSP completions ──────────────────────────────────────────────────────

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
