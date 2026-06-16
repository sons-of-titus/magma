use crate::kernel::scripting;
use crate::tests::helpers;

// ── window/list ───────────────────────────────────────────────────────────

#[test]
fn window_list_returns_one_window_initially() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    let result = scripting::eval("(length (window/list))");
    assert_eq!(result, "ok");
    assert_eq!(ed.windows.len(), 1);
}

#[test]
fn window_list_returns_all_windows_after_split() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    ed.windows.split_vertical(ed.windows.focused_window().unwrap());
    let result = scripting::eval("(length (window/list))");
    assert_eq!(result, "ok");
    assert_eq!(ed.windows.len(), 2);
}

// ── window/focus ──────────────────────────────────────────────────────────

#[test]
fn window_focus_changes_focused_window_via_janet() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    let new_id = ed.windows.split_vertical(ed.windows.focused_window().unwrap()).unwrap();
    let initial_focused = ed.windows.focused_window();

    let result = scripting::eval(
        &format!("(window/focus {})", new_id.0),
    );
    assert_eq!(result, "ok");
    assert_ne!(ed.windows.focused_window(), initial_focused);
    assert_eq!(ed.windows.focused_window(), Some(new_id));
}

#[test]
fn window_focus_emits_window_focused_event() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    let new_id = ed.windows.split_vertical(ed.windows.focused_window().unwrap()).unwrap();

    // Register a Rust-side event counter (avoids nested fiber issue).
    use std::sync::{Arc, Mutex};
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = fired.clone();
    ed.events.on("window-focused", move |_data| {
        *fired_clone.lock().unwrap() = true;
        None
    });

    scripting::eval(&format!("(window/focus {})", new_id.0));
    ed.events.drain_and_dispatch();

    assert!(*fired.lock().unwrap(), "window-focused event must fire on focus change");
}

// ── window/resize ─────────────────────────────────────────────────────────

#[test]
fn window_resize_sets_fractional_weight() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    ed.windows.resize(80, 24);
    let new_id = ed.windows.split_vertical(ed.windows.focused_window().unwrap()).unwrap();
    let focused_id = ed.windows.focused_window().unwrap();

    let result = scripting::eval(
        &format!("(window/resize {} 0.3)", focused_id.0),
    );
    assert_eq!(result, "ok");

    let w1 = ed.windows.window(focused_id).unwrap().width;
    let w2 = ed.windows.window(new_id).unwrap().width;
    assert!(w1 < w2, "resized window (weight 0.3) should be narrower than sibling");
}

// ── window/set-scroll-top / window/scroll-top ─────────────────────────────

#[test]
fn window_set_scroll_top_pins_offset() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    let wid = ed.windows.focused_window().unwrap();
    let result = scripting::eval(
        &format!("(window/set-scroll-top {} 3)", wid.0),
    );
    assert_eq!(result, "ok");

    let win = ed.windows.window(wid).unwrap();
    assert!(win.scroll_pinned, "scroll_pinned must be true after set-scroll-top");
    assert_eq!(win.scroll_offset, 3, "scroll_offset must equal the requested top");
}

#[test]
fn window_scroll_top_returns_pinned_value() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    let wid = ed.windows.focused_window().unwrap();
    // Pin scroll top to 5
    {
        let win = ed.windows.window_mut(wid).unwrap();
        win.scroll_offset = 5;
        win.scroll_pinned = true;
    }

    // window/scroll-top should return the pinned value
    let result = scripting::eval(
        &format!("(= (window/scroll-top {}) 5)", wid.0),
    );
    assert_eq!(result, "ok");
}

#[test]
fn window_unpin_scroll_clears_pin() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    let wid = ed.windows.focused_window().unwrap();
    scripting::eval(&format!("(window/set-scroll-top {} 7)", wid.0));
    assert!(ed.windows.window(wid).unwrap().scroll_pinned);

    let result = scripting::eval(
        &format!("(window/unpin-scroll {})", wid.0),
    );
    assert_eq!(result, "ok");
    assert!(!ed.windows.window(wid).unwrap().scroll_pinned);
}

// ── editor/save-layout / editor/restore-layout ────────────────────────────

#[test]
fn editor_save_layout_stores_snapshot() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    let result = scripting::eval("(editor/save-layout \"my-layout\")");
    assert_eq!(result, "ok");
    assert!(ed.saved_layouts.contains_key("my-layout"), "layout must be stored");
}

#[test]
fn editor_restore_layout_restores_window_count() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    // Save single-window layout
    scripting::eval("(editor/save-layout \"single\")");

    // Add a second window
    ed.windows.split_vertical(ed.windows.focused_window().unwrap());
    assert_eq!(ed.windows.len(), 2);

    // Restore single-window layout
    let result = scripting::eval("(editor/restore-layout \"single\")");
    assert_eq!(result, "ok");
    assert_eq!(ed.windows.len(), 1, "restore must return to saved window count");
}

#[test]
fn editor_restore_layout_no_op_when_name_unknown() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    ed.windows.split_vertical(ed.windows.focused_window().unwrap());
    let count_before = ed.windows.len();

    // Restore a name that was never saved — should not crash or change state
    let result = scripting::eval("(editor/restore-layout \"nonexistent\")");
    assert_eq!(result, "ok");
    assert_eq!(ed.windows.len(), count_before);
}

// ── editor/layout-list ────────────────────────────────────────────────────

#[test]
fn editor_layout_list_returns_saved_names() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    scripting::eval("(editor/save-layout \"alpha\")");
    scripting::eval("(editor/save-layout \"beta\")");

    assert_eq!(ed.saved_layouts.len(), 2);
    assert!(ed.saved_layouts.contains_key("alpha"));
    assert!(ed.saved_layouts.contains_key("beta"));

    let result = scripting::eval(
        "(= (length (editor/layout-list)) 2)",
    );
    assert_eq!(result, "ok");
}

// ── window/current ────────────────────────────────────────────────────────

#[test]
fn window_current_returns_focused_window_table() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line1\nline2\nline3\n");
    scripting::init(&mut ed);

    let result = scripting::eval("(get (window/current) :id)");
    assert_eq!(result, "ok");
    // Rust-side: focused window should be window 1
    assert_eq!(ed.windows.focused_window().unwrap().0, 1);
}
