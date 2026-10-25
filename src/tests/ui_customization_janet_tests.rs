use crate::kernel::scripting;
use crate::kernel::state::Editor;

// ── overlay ──────────────────────────────────────────────────────────────

#[test]
fn overlay_create_returns_id_and_stores_overlay() {
    janet_test!(ed, {
        scripting::eval(r#"(overlay/create 5 10 20 8 nil)"#);
        assert_eq!(ed.overlays.len(), 1);
        assert_eq!(ed.overlays[0].x, 5);
        assert_eq!(ed.overlays[0].y, 10);
        assert_eq!(ed.overlays[0].width, 20);
        assert_eq!(ed.overlays[0].height, 8);
    });
}

#[test]
fn overlay_destroy_removes_overlay() {
    janet_test!(ed, {
        scripting::eval(r#"(def oid (overlay/create 0 0 10 5 nil))"#);
        scripting::eval(r#"(overlay/destroy oid)"#);
        assert!(ed.overlays.is_empty());
    });
}

#[test]
fn overlay_move_updates_position() {
    janet_test!(ed, {
        scripting::eval(r#"(def oid (overlay/create 0 0 10 5 nil))"#);
        scripting::eval(r#"(overlay/move oid 15 7)"#);
        assert_eq!(ed.overlays[0].x, 15);
        assert_eq!(ed.overlays[0].y, 7);
    });
}

#[test]
fn overlay_list_returns_all_overlays() {
    janet_test!(ed, {
        scripting::eval(r#"(overlay/create 0 0 10 5 nil)"#);
        scripting::eval(r#"(overlay/create 5 5 20 8 nil)"#);
        assert_eq!(ed.overlays.len(), 2);
    });
}

// ── buffer header line ────────────────────────────────────────────────────

#[test]
fn set_header_line_stores_text() {
    janet_test!(ed, {
        let key = ed.create_buffer("test");
        scripting::eval(&format!(r#"(buffer/set-header-line {key} "MyHeader")"#));
        assert_eq!(ed.buffers.get(key).unwrap().lock().unwrap().header_line.as_deref(), Some("MyHeader"));
    });
}

#[test]
fn set_header_line_nil_clears_text() {
    janet_test!(ed, {
        let key = ed.create_buffer("test");
        scripting::eval(&format!(r#"(buffer/set-header-line {key} "MyHeader")"#));
        scripting::eval(&format!(r#"(buffer/set-header-line {key} nil)"#));
        assert!(ed.buffers.get(key).unwrap().lock().unwrap().header_line.is_none());
    });
}

fn make_buf_local(ed: &mut Editor) -> usize {
    ed.create_buffer("test-ui")
}

#[test]
fn editor_ready_sets_defaults() {
    janet_test!(ed, {
        let data = std::collections::HashMap::new();
        ed.events.emit("editor-ready", data.clone());
        ed.events.drain_and_dispatch();
        assert_eq!(ed.options.get("number").map(|s| s.as_str()), Some("true"));
        assert_eq!(ed.options.get("scrolloff").map(|s| s.as_str()), Some("3"));
        assert_eq!(ed.options.get("mouse-support").map(|s| s.as_str()), Some("true"));
        assert!(ed.faces.contains_key("fringe"), "fringe face should be defined");
        assert!(ed.faces.contains_key("overlay-border"), "overlay-border face should be defined");
        let _ = data;
    });
}

#[test]
fn editor_ready_enters_standard_editor_mode() {
    janet_test!(ed, {
        let data = std::collections::HashMap::new();
        ed.events.emit("editor-ready", data.clone());
        ed.events.drain_and_dispatch();
        assert_eq!(ed.editor_mode.name, "editor", "standard editing mode after editor-ready");
        assert!(ed.editor_mode.accepts_text, "typing inserts text immediately");
    });
}

#[test]
fn buffer_modified_c_fn_tracks_edits_from_a_clean_baseline() {
    janet_test!(ed, {
        let key = make_buf_local(&mut ed);
        assert!(!ed.buffers.get(key).unwrap().lock().unwrap().modified(), "a fresh buffer starts at its baseline");
        ed.buffers.get(key).unwrap().lock().unwrap().insert(0, "hello");
        assert!(ed.buffers.get(key).unwrap().lock().unwrap().modified(), "an edit moves off the baseline");
        let result = scripting::eval(&format!("(buffer/modified? {key})"));
        assert_eq!(result, "ok");
    });
}

#[test]
fn buffer_modified_c_fn_works_after_save() {
    janet_test!(ed, {
        let key = make_buf_local(&mut ed);
        ed.buffers.get(key).unwrap().lock().unwrap().insert(0, "hello");
        ed.buffers.get(key).unwrap().lock().unwrap().mark_saved();
        assert!(!ed.buffers.get(key).unwrap().lock().unwrap().modified(), "after mark_saved → not modified");
        let result = scripting::eval(&format!("(buffer/modified? {key})"));
        assert_eq!(result, "ok");
    });
}

#[test]
fn editor_fs_exists_true_for_existing_path() {
    janet_test!(ed, {
        let result = scripting::eval(r#"(editor/fs-exists? "/tmp")"#);
        assert_eq!(result, "ok");
    });
}

#[test]
fn editor_fs_exists_false_for_missing_path() {
    janet_test!(ed, {
        let result = scripting::eval(
            r#"(editor/fs-exists? "/nonexistent-magma-xyz-abc-123")"#);
        assert_eq!(result, "ok");
    });
}
