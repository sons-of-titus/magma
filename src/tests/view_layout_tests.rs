//! Layout behavior — split axis, pane resize, last-tab close.

use crate::kernel::state::id::WindowId;
use crate::kernel::render::view_tree::ViewTree;
use crate::kernel::render::gui_tabs::TabAction;
use crate::tests::helpers::make_editor;

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
fn closing_the_last_tab_lands_on_scratch() {
    let mut ed = make_editor();
    TabAction::Close(WindowId(1)).apply(&mut ed);
    let buf = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .and_then(|key| ed.buffers.get(key))
        .map(|arc| arc.lock().unwrap().name.clone());
    assert_eq!(buf.as_deref(), Some("*scratch*"), "there is always somewhere to type");
}
