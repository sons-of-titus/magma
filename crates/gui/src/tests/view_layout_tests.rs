//! Layout behavior — split axis, pane resize, last-tab close.

use kernel::state::id::WindowId;
use crate::render::view_tree::ViewTree;
use crate::render::gui_tabs::TabAction;
use crate::tests::helpers::{self, make_editor};

#[test]
fn vertical_split_stacks_panes() {
    let mut vt = ViewTree::new();
    vt.resize(120, 40);
    let first = vt.focused_window().unwrap();
    vt.split_vertical(first);
    let panes = vt.panes();
    assert_eq!(panes.len(), 2);
    assert_eq!(panes[0].width, 120, "vertical split keeps full width");
    assert_eq!(panes[1].width, 120);
    // One row is reserved by redistribute (status strip, TUI heritage).
    assert_eq!(panes[0].height + panes[1].height, 39, "height is divided");
    assert_eq!(panes[0].y, 0);
    assert_eq!(panes[1].y, panes[0].height, "second pane stacks below");
}

#[test]
fn horizontal_split_still_tiles_rows() {
    let mut vt = ViewTree::new();
    vt.resize(120, 40);
    let first = vt.focused_window().unwrap();
    vt.split_horizontal(first);
    let panes = vt.panes();
    assert_eq!(panes[0].height, 39, "full column height minus the reserved row");
    assert!(panes[1].x > 0, "second pane sits to the right");
}

#[test]
fn resize_moves_weight_between_panes() {
    let mut vt = ViewTree::new();
    vt.resize(100, 40);
    let first = vt.focused_window().unwrap();
    let second = vt.split_horizontal(first).unwrap();
    vt.focus(second);
    vt.resize_weighted(second, 0.8);
    let panes = vt.panes();
    let focused = panes.iter().find(|p| p.id == second).unwrap();
    assert!(focused.width > panes.iter().filter(|p| p.id != second).map(|p| p.width).sum::<u16>(),
        "an 0.8-weight pane holds most of the row");
}

#[test]
fn closing_the_last_tab_leaves_an_empty_editor() {
    let mut ed = make_editor();
    TabAction::Close(WindowId(1)).apply(&mut ed);
    let buf = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .and_then(|key| ed.buffers.get(key))
        .map(|arc| arc.lock().unwrap().name.clone());
    assert!(buf.is_none(),
        "closing the last tab leaves an empty editor area, not a scratch buffer");
}

// ── Tab layout — files open as stacked tabs, not over the current pane ──────

#[test]
fn open_tab_adds_full_size_focused_pane() {
    let mut tree = ViewTree::new();
    let first = tree.focused_window().unwrap();
    let second = tree.open_tab(None);

    assert_ne!(first, second);
    assert_eq!(tree.focused_window(), Some(second), "the new tab takes focus");
    let pane = tree.pane(second).unwrap();
    assert_eq!(pane.layout, crate::render::view_tree::LayoutConstraint::Tab);
    assert_eq!(pane.width, tree.pane(first).unwrap().width,
        "tab panes stack at full editor size");
    assert_eq!(pane.x, 0);
    assert_eq!(tree.pane(first).unwrap().width, pane.width,
        "the first tab keeps its full size too");
}

#[test]
fn closing_a_tab_keeps_the_stack_layout() {
    let mut tree = ViewTree::new();
    let _a = tree.focused_window().unwrap();
    let b = tree.open_tab(None);
    let c = tree.open_tab(None);
    assert_eq!(tree.panes().len(), 3);
    tree.close(b);
    assert_eq!(tree.panes().len(), 2);
    assert_eq!(tree.focused_window(), Some(c), "focus stays on the visible tab");
    let width = tree.pane(tree.focused_window().unwrap()).unwrap().width;
    assert!(width > 0, "tab panes keep a real width");
    for pane in tree.panes() {
        assert_eq!(pane.width, width, "every remaining tab stays full size");
    }
}

#[test]
fn open_file_lands_in_a_new_tab_and_focuses_it() {
    let mut ed = make_editor();
    let dir = std::env::temp_dir().join("magma_open_tab_test");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("tabbed.rs");
    std::fs::write(&path, "pub fn main() {}\n").unwrap();

    // Dirty the current tab so the open cannot reuse it as untouched scratch.
    let cur = helpers::focused_key(&ed);
    if let Some(view) = ed.view_of_buffer_mut(cur) {
        view.insert(0, "occupied");
    }

    let panes_before = ed.view_tree.panes().len();
    let mut args = std::collections::HashMap::new();
    args.insert("path".to_string(),
        kernel::command::args::ArgValue::Path(path.to_string_lossy().into_owned()));
    kernel::command::execute_command(&mut ed, "open-file", &args).unwrap();

    assert_eq!(ed.view_tree.panes().len(), panes_before + 1,
        "opening a file adds a tab");
    let opened = helpers::focused_key(&ed);
    let opened_path = ed.buffers.get(opened).unwrap().lock().unwrap().path.clone();
    assert_eq!(opened_path.as_deref(), Some(path.to_string_lossy().as_ref()));
}

#[test]
fn opening_a_file_fills_a_bufferless_pane_instead_of_stacking() {
    // The empty-editor startup state: one pane, no buffer.
    let mut ed = make_editor();
    if let Some(w) = ed.view_tree.focused_window_mut() { w.buffer_id = None; }
    let panes_before = ed.view_tree.panes().len();

    let dir = std::env::temp_dir().join("magma_open_empty_pane");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("first.rs");
    std::fs::write(&path, "// first\n").unwrap();

    let mut args = std::collections::HashMap::new();
    args.insert("path".to_string(),
        kernel::command::args::ArgValue::Path(path.to_string_lossy().into_owned()));
    kernel::command::execute_command(&mut ed, "open-file", &args).unwrap();

    assert_eq!(ed.view_tree.panes().len(), panes_before,
        "an empty editor pane is filled, not stacked over");
    let opened = helpers::focused_key(&ed);
    assert_ne!(opened, 0, "the pane now shows the opened file");
    assert_eq!(ed.buffers.get(opened).unwrap().lock().unwrap().path.as_deref(),
        Some(path.to_string_lossy().as_ref()));
}

#[test]
fn open_file_replaces_untouched_scratch_instead_of_stacking() {
    let mut ed = make_editor(); // focused buffer: empty "*test*" scratch
    let dir = std::env::temp_dir().join("magma_open_scratch_test");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("first.rs");
    std::fs::write(&path, "// first\n").unwrap();

    let panes_before = ed.view_tree.panes().len();
    let mut args = std::collections::HashMap::new();
    args.insert("path".to_string(),
        kernel::command::args::ArgValue::Path(path.to_string_lossy().into_owned()));
    kernel::command::execute_command(&mut ed, "open-file", &args).unwrap();

    assert_eq!(ed.view_tree.panes().len(), panes_before,
        "an empty untouched scratch is replaced, not stacked over");
}
