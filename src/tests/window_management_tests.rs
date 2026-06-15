use crate::buffer::Buffer;
use crate::command::builtin;
use crate::fs::disk::DiskFileSystem;
use crate::state::id::{BufferId, WindowId};
use crate::state::Editor;

fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::from_string(BufferId(id), "test", "hello\nworld\n");
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() { win.buffer_id = Some(key); }
    ed
}

// ── Window split and count ─────────────────────────────────────────────────

#[test]
fn window_split_increases_window_count() {
    let mut ed = make_editor();
    assert_eq!(ed.windows.len(), 1);
    let wid = ed.windows.focused_window().unwrap();
    ed.windows.split_vertical(wid);
    assert_eq!(ed.windows.len(), 2);
}

#[test]
fn window_split_horizontal_increases_window_count() {
    let mut ed = make_editor();
    let wid = ed.windows.focused_window().unwrap();
    ed.windows.split_horizontal(wid);
    assert_eq!(ed.windows.len(), 2);
}

// ── Window focus ──────────────────────────────────────────────────────────

#[test]
fn window_focus_changes_focused_window() {
    let mut ed = make_editor();
    let focused = ed.windows.focused_window().unwrap();
    let new_id = ed.windows.split_vertical(focused).unwrap();
    let initial_focused = ed.windows.focused_window();
    ed.windows.focus(new_id);
    let after_focused = ed.windows.focused_window();
    assert_ne!(initial_focused, after_focused);
    assert_eq!(after_focused, Some(new_id));
}

#[test]
fn window_focus_next_cycles_through_windows() {
    let mut ed = make_editor();
    let wid = ed.windows.focused_window().unwrap();
    ed.windows.split_vertical(wid);
    let first = ed.windows.focused_window();
    ed.windows.focus_next();
    let second = ed.windows.focused_window();
    assert_ne!(first, second);
    ed.windows.focus_next();
    let third = ed.windows.focused_window();
    assert_eq!(first, third);
}

// ── Window resize (fractional weight) ─────────────────────────────────────

#[test]
fn window_resize_weighted_redistributes_widths() {
    let mut ed = make_editor();
    ed.windows.resize(80, 24);
    let focused = ed.windows.focused_window().unwrap();
    let new_id = ed.windows.split_vertical(focused).unwrap();

    let focused_id = ed.windows.focused_window().unwrap();
    ed.windows.resize_weighted(focused_id, 0.3);

    let w1 = ed.windows.window(focused_id).unwrap().width;
    let w2 = ed.windows.window(new_id).unwrap().width;
    assert!(w1 < w2, "left window (weight 0.3) should be narrower than right (weight 1.0)");
    assert!((w1 as u32 + w2 as u32).abs_diff(80) <= 1, "widths should sum to terminal width");
}

#[test]
fn window_resize_weighted_single_window_fills_terminal() {
    let mut ed = make_editor();
    ed.windows.resize(100, 30);
    let wid = ed.windows.focused_window().unwrap();
    ed.windows.resize_weighted(wid, 0.5);
    let w = ed.windows.window(wid).unwrap().width;
    assert_eq!(w, 100, "single window always fills terminal regardless of weight");
}

// ── Per-window scroll state ────────────────────────────────────────────────

#[test]
fn window_scroll_pinned_defaults_to_false() {
    let ed = make_editor();
    let wid = ed.windows.focused_window().unwrap();
    assert!(!ed.windows.window(wid).unwrap().scroll_pinned);
}

#[test]
fn window_scroll_offset_stores_explicit_top() {
    let mut ed = make_editor();
    let wid = ed.windows.focused_window().unwrap();
    let win = ed.windows.window_mut(wid).unwrap();
    win.scroll_offset = 5;
    win.scroll_pinned = true;
    assert_eq!(ed.windows.window(wid).unwrap().scroll_offset, 5);
    assert!(ed.windows.window(wid).unwrap().scroll_pinned);
}

#[test]
fn window_unpin_clears_scroll_pinned() {
    let mut ed = make_editor();
    let wid = ed.windows.focused_window().unwrap();
    {
        let win = ed.windows.window_mut(wid).unwrap();
        win.scroll_offset = 10;
        win.scroll_pinned = true;
    }
    ed.windows.window_mut(wid).unwrap().scroll_pinned = false;
    assert!(!ed.windows.window(wid).unwrap().scroll_pinned);
}

// ── Named layouts ─────────────────────────────────────────────────────────

#[test]
fn save_layout_records_window_state() {
    let mut ed = make_editor();
    let wid = ed.windows.focused_window().unwrap();
    ed.windows.split_vertical(wid);
    assert_eq!(ed.windows.len(), 2);
    let wins = ed.windows.windows().to_vec();
    let focused_idx = ed.windows.focused_index();
    ed.saved_layouts.insert("two-pane".to_string(), (wins, focused_idx));
    assert!(ed.saved_layouts.contains_key("two-pane"));
}

#[test]
fn restore_layout_restores_window_count() {
    let mut ed = make_editor();
    let wins = ed.windows.windows().to_vec();
    let fi = ed.windows.focused_index();
    ed.saved_layouts.insert("single".to_string(), (wins, fi));

    let wid = ed.windows.focused_window().unwrap();
    ed.windows.split_vertical(wid);
    assert_eq!(ed.windows.len(), 2);

    let (saved_wins, saved_fi) = ed.saved_layouts["single"].clone();
    ed.windows.restore(saved_wins, saved_fi);
    assert_eq!(ed.windows.len(), 1);
}

#[test]
fn restore_layout_restores_focused_index() {
    let mut ed = make_editor();
    let focused = ed.windows.focused_window().unwrap();
    let new_id = ed.windows.split_vertical(focused).unwrap();
    ed.windows.focus(new_id);

    let wins = ed.windows.windows().to_vec();
    let fi = ed.windows.focused_index();
    ed.saved_layouts.insert("saved".to_string(), (wins, fi));

    let first_id = WindowId::from_u64(1);
    ed.windows.focus(first_id);

    let (saved_wins, saved_fi) = ed.saved_layouts["saved"].clone();
    ed.windows.restore(saved_wins, saved_fi);
    assert_eq!(ed.windows.focused_window(), Some(new_id));
}

// ── Window close ──────────────────────────────────────────────────────────

#[test]
fn window_close_removes_window() {
    let mut ed = make_editor();
    let focused = ed.windows.focused_window().unwrap();
    let new_id = ed.windows.split_vertical(focused).unwrap();
    assert_eq!(ed.windows.len(), 2);
    ed.windows.close(new_id);
    assert_eq!(ed.windows.len(), 1);
}

#[test]
fn window_close_does_not_remove_last_window() {
    let mut ed = make_editor();
    let only_id = ed.windows.focused_window().unwrap();
    ed.windows.close(only_id);
    assert_eq!(ed.windows.len(), 1, "cannot close the last window");
}

// ── Resize and terminal dimensions ────────────────────────────────────────

#[test]
fn resize_updates_single_window_dimensions() {
    let mut ed = make_editor();
    ed.windows.resize(120, 40);
    let wid = ed.windows.focused_window().unwrap();
    let win = ed.windows.window(wid).unwrap();
    assert_eq!(win.width, 120);
    assert_eq!(win.height, 39); // term_height - 1 (status bar)
}

#[test]
fn window_weight_defaults_to_one() {
    let ed = make_editor();
    let wid = ed.windows.focused_window().unwrap();
    assert!((ed.windows.window(wid).unwrap().weight - 1.0).abs() < 1e-6);
}
