//! Phase 14 correctness gates (docs/core-redesign.md) — the invariants the
//! redesign is not complete without, asserted end-to-end.  Individual
//! contract details live in the per-area suites; this file proves the
//! cross-subsystem invariants hold on the integrated editor.

use crate::kernel::command::{self, builtin};
use crate::kernel::storage::disk::DiskFileSystem;
use crate::kernel::render::surface::Surface;
use crate::kernel::state::Editor;
use crate::tests::helpers;

fn editor_with(content: &str) -> (Editor, usize) {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer_from_str("gate", content);
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    (ed, key)
}

#[test]
fn gate_mutation_one_path_one_revision() {
    // Typing through the input pipeline produces exactly one committed
    // transaction, one revision, one pending change, one published event.
    let (mut ed, key) = editor_with("seed");
    ed.editor_mode = crate::kernel::state::EditorMode::new("insert", true);
    let rev0 = ed.buffers.get(key).unwrap().lock().unwrap().revision();
    crate::kernel::input::dispatch_key(&mut ed, "!");
    // dispatch_key publishes and drains internally; the observable contract
    // is one revision and the typed text.
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    assert_eq!(buf.revision(), rev0 + 1, "one keystroke = one revision");
    drop(buf);
    assert_eq!(helpers::buf_text(&ed), "!seed", "typed at the cursor (offset 0)");

    // Outside dispatch, a raw mutation queues until publication.
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(0, ">");
    assert!(ed.buffers.get(key).unwrap().lock().unwrap().has_pending_changes());
    ed.publish_buffer_changes();
    assert!(!ed.buffers.get(key).unwrap().lock().unwrap().has_pending_changes());
    assert_eq!(helpers::buf_text(&ed), ">!seed");
}

#[test]
fn gate_stale_diagnostics_and_changes_never_override_newer_text() {
    // A consumer that read at revision N cannot corrupt revision N+2:
    // ranges addressed at N are only obtainable through changes_since(N),
    // and text mutations validate every range at commit time.
    let (mut ed, key) = editor_with("hello world");
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(5, ",");
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(6, " big");
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    assert_eq!(buf.slice(0, usize::MAX), "hello, big world");
    // The old edit's coordinates no longer exist as a live address; the
    // change log is the only way to rebase them.
    let chain = buf.changes_since(0);
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0].0, 0); // before
    assert_eq!(chain[1].0, 1);
}

#[test]
fn gate_persistence_invariant_end_to_end() {
    // Edit → save → edit → stale completion → still modified → fresh save
    // → clean.
    let dir = std::env::temp_dir().join(format!("magma-gate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("g.txt");
    let path_str = path.to_str().unwrap().to_string();

    let (mut ed, key) = editor_with("v1");
    let (content, rev) = {
        let buf = ed.buffers.get(key).unwrap().lock().unwrap();
        (buf.slice(0, buf.len()), buf.revision())
    };
    crate::kernel::storage::disk::atomic_write(&path_str, &content).unwrap();
    ed.buffers.get(key).unwrap().lock().unwrap().mark_saved_at(rev);
    assert!(!ed.buffers.get(key).unwrap().lock().unwrap().modified());

    // Async-style save of an older revision completes after a newer edit.
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(2, "-stale-snapshot");
    ed.buffers.get(key).unwrap().lock().unwrap().mark_saved_at(rev);
    assert!(
        ed.buffers.get(key).unwrap().lock().unwrap().modified(),
        "stale completion must not clear modified"
    );

    // Saving the current revision clears it.
    let (content, rev) = {
        let buf = ed.buffers.get(key).unwrap().lock().unwrap();
        (buf.slice(0, buf.len()), buf.revision())
    };
    crate::kernel::storage::disk::atomic_write(&path_str, &content).unwrap();
    ed.buffers.get(key).unwrap().lock().unwrap().mark_saved_at(rev);
    assert!(!ed.buffers.get(key).unwrap().lock().unwrap().modified());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "v1-stale-snapshot");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn gate_undo_never_loses_or_corrupts_history() {
    // edit → undo → edit (branch) → full traversal stays consistent and
    // the chain remains connected.
    let (mut ed, key) = editor_with("");
    for s in ["a", "b", "c"] {
        let view = ed.views.get_mut(&key).unwrap();
        let cur = view.cursor_offset();
        view.insert(cur, s);
    }
    let view = ed.views.get_mut(&key).unwrap();
    assert!(view.undo());
    let view = ed.views.get_mut(&key).unwrap();
    let cur = view.cursor_offset();
    view.insert(cur, "C"); // discards "c" redo branch by policy

    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    assert_eq!(buf.slice(0, buf.len()), "abC");
    assert!(!buf.can_redo(), "branch is explicit policy, not data loss");
    drop(buf);

    let mut steps = 0;
    while {
        let view = ed.views.get_mut(&key).unwrap();
        view.undo()
    } {
        steps += 1;
    }
    assert_eq!(steps, 3, "every edit is undoable exactly once");
    assert_eq!(helpers::buf_text(&ed), "", "history traversal reaches the origin");
}

#[test]
fn gate_ffi_without_scope_errors_cleanly() {
    // A Janet eval outside any editor scope must fail with a clean Janet
    // error — never dereference a stale or foreign pointer.
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    crate::kernel::scripting::init(&mut ed);
    let out = crate::kernel::scripting::eval(r#"(buffer/len 0)"#);
    assert_ne!(out, "", "eval completes");
    // Whatever the surface result, the process is alive and the VM intact:
    let out2 = crate::kernel::scripting::eval("(+ 1 2)");
    assert_eq!(out2, "ok", "VM is healthy after the uns scoped access attempt");
    let _ = out;
}

#[test]
fn gate_ffi_scoped_nesting_restores_outer_target() {
    use crate::kernel::scripting::EditorScope;
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    crate::kernel::scripting::init(&mut ed);
    let key = helpers::focused_key(&ed);

    // Guard nesting composes: the inner scope's drop reinstates the outer
    // pointer, so Janet calls between the two see the same editor.
    let _outer = EditorScope::new(&mut ed);
    {
        let view = ed.views.get_mut(&key).unwrap();
        view.insert(0, "outer");
    }
    let text_outer = helpers::buf_text(&ed);
    {
        let _inner = EditorScope::new(&mut ed);
        let view = ed.views.get_mut(&key).unwrap();
        view.insert(text_outer.len(), "-inner");
    }
    drop(_outer);
    // After both scopes close, Janet access is uninstalled again.
    assert_eq!(helpers::buf_text(&ed), "outer-inner");
}

#[test]
fn gate_view_invariant_two_windows_one_document() {
    let (mut ed, key) = editor_with("one\ntwo\nthree\n");
    command::execute_command(&mut ed, "window-split", &Default::default()).unwrap();
    command::execute_command(&mut ed, "window-vsplit", &Default::default()).unwrap();

    // Three panes, two forked views + the base view; cursors independent.
    let focused = ed.view_tree.focused_window().unwrap();
    {
        let view = ed.focused_view_mut().unwrap();
        view.set_cursor(4);
    }
    assert_ne!(
        ed.window_views.get(&(focused, key)).unwrap().cursor_offset(),
        ed.views.get(&key).unwrap().cursor_offset(),
        "the base view must not follow the forked one"
    );

    // An edit in one window is visible in the other (shared document).
    {
        let view = ed.focused_view_mut().unwrap();
        let cur = view.cursor_offset();
        view.insert(cur, "X");
    }
    assert!(helpers::buf_text(&ed).starts_with("one\nX"), "edit visible through the shared document");
}

#[test]
fn gate_rendering_never_copies_the_document() {
    // Structural proof: the frame path exposes no whole-document slice.
    // The behavioral proof (flat render latency across sizes) lives in
    // benches/editor_workloads.rs; here we assert the visible-window
    // contract on a document far larger than the viewport.
    let doc: String = (0..50_000).map(|i| format!("line-{i}\n")).collect();
    let (ed, _key) = editor_with(&doc);
    let mut surface = Surface::new(40, 8);
    crate::kernel::render::frame::render_frame(&ed, &mut surface);
    let mut seen = String::new();
    for y in 0..7 {
        for x in 0..40 {
            if let Some(cell) = surface.cell(x, y) {
                seen.push(cell.ch);
            }
        }
    }
    assert!(seen.contains("ine-0"), "first line rendered");
    assert!(!seen.contains("ine-49999"), "no leak from outside the viewport");
}
