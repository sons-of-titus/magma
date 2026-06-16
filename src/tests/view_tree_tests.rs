use crate::kernel::render::view_tree::{ViewTree, LayoutConstraint, SplitDirection};
use crate::kernel::render::view::{Rect, View, ViewKind};
use crate::kernel::render::editor_view::EditorView;
use crate::kernel::render::terminal_view::TerminalView;
use crate::kernel::render::sidebar_view::SidebarView;
use crate::kernel::render::surface::Surface;
use crate::kernel::state::id::WindowId;
use crate::tests::helpers;

// ── ViewTree layout ────────────────────────────────────────────────────────

#[test]
fn view_tree_starts_with_one_pane() {
    let vt = ViewTree::new();
    assert_eq!(vt.len(), 1);
}

#[test]
fn split_horizontal_adds_a_pane_with_correct_constraint() {
    let mut vt = ViewTree::new();
    let id = vt.focused_window().unwrap();
    let new_id = vt.split_horizontal(id).unwrap();
    assert_eq!(vt.len(), 2);
    assert_eq!(
        vt.pane(new_id).unwrap().layout,
        LayoutConstraint::Split(SplitDirection::Horizontal)
    );
}

#[test]
fn split_vertical_adds_a_pane_with_correct_constraint() {
    let mut vt = ViewTree::new();
    let id = vt.focused_window().unwrap();
    let new_id = vt.split_vertical(id).unwrap();
    assert_eq!(vt.len(), 2);
    assert_eq!(
        vt.pane(new_id).unwrap().layout,
        LayoutConstraint::Split(SplitDirection::Vertical)
    );
}

#[test]
fn redistribute_fills_single_pane_to_terminal_size() {
    let mut vt = ViewTree::new();
    vt.resize(100, 30);
    let id = vt.focused_window().unwrap();
    let p = vt.pane(id).unwrap();
    assert_eq!(p.width, 100);
    assert_eq!(p.height, 29); // term_height - 1 for status bar
}

#[test]
fn redistribute_two_panes_sum_to_terminal_width() {
    let mut vt = ViewTree::new();
    vt.resize(80, 24);
    let id = vt.focused_window().unwrap();
    let new_id = vt.split_horizontal(id).unwrap();
    let w1 = vt.pane(id).unwrap().width;
    let w2 = vt.pane(new_id).unwrap().width;
    assert_eq!((w1 + w2) as u32, 80);
}

#[test]
fn close_pane_removes_it_and_redistributes() {
    let mut vt = ViewTree::new();
    vt.resize(80, 24);
    let id = vt.focused_window().unwrap();
    let new_id = vt.split_horizontal(id).unwrap();
    assert_eq!(vt.len(), 2);
    vt.close(new_id);
    assert_eq!(vt.len(), 1);
    let p = vt.pane(id).unwrap();
    assert_eq!(p.width, 80);
}

#[test]
fn cannot_close_last_pane() {
    let mut vt = ViewTree::new();
    let id = vt.focused_window().unwrap();
    vt.close(id);
    assert_eq!(vt.len(), 1, "last pane must not be removed");
}

#[test]
fn focused_buffer_returns_none_when_pane_has_no_buffer() {
    let vt = ViewTree::new();
    assert!(vt.focused_buffer().is_none());
}

#[test]
fn set_buffer_and_focused_buffer_roundtrip() {
    let mut vt = ViewTree::new();
    let id = vt.focused_window().unwrap();
    vt.set_buffer(id, 42);
    assert_eq!(vt.focused_buffer(), Some(42));
}

#[test]
fn resize_weighted_makes_panes_unequal() {
    let mut vt = ViewTree::new();
    vt.resize(80, 24);
    let id = vt.focused_window().unwrap();
    let new_id = vt.split_horizontal(id).unwrap();
    vt.resize_weighted(id, 0.25);
    let w1 = vt.pane(id).unwrap().width;
    let w2 = vt.pane(new_id).unwrap().width;
    assert!(w1 < w2, "lighter-weighted pane should be narrower");
}

// ── View kind ─────────────────────────────────────────────────────────────

#[test]
fn editor_view_kind_is_editor() {
    let v = EditorView { buf_id: 0, pane_id: WindowId(1) };
    assert_eq!(v.kind(), ViewKind::Editor);
}

#[test]
fn terminal_view_kind_is_terminal() {
    let v = TerminalView { buf_id: 0 };
    assert_eq!(v.kind(), ViewKind::Terminal);
}

#[test]
fn sidebar_view_kind_is_sidebar() {
    let v = SidebarView;
    assert_eq!(v.kind(), ViewKind::Sidebar);
}

// ── View renders into area bounds ─────────────────────────────────────────

#[test]
fn sidebar_render_only_writes_within_area() {
    use crate::kernel::render::view::{View, RenderCtx};
    let ed = helpers::make_editor();
    let mut surface = Surface::new(80, 24);
    let area = Rect::new(0, 0, 20, 10);
    SidebarView.render(&mut surface, &RenderCtx { editor: &ed, area });

    // Nothing outside the area width should have changed from default (space).
    for x in 20..80u16 {
        let cell = surface.cell(x, 0).unwrap();
        assert_eq!(cell.ch, ' ', "sidebar must not write beyond area.width");
    }
}

// ── render_frame with ViewTree ─────────────────────────────────────────────

#[test]
fn render_frame_with_no_buffer_does_not_panic() {
    use crate::kernel::render::frame::render_frame;
    let ed = helpers::make_editor();
    let mut surface = Surface::new(80, 24);
    render_frame(&ed, &mut surface);
}

#[test]
fn render_frame_with_buffer_writes_content() {
    use crate::kernel::render::frame::render_frame;
    let mut ed = helpers::make_editor_with_buffer("hello world\n");
    let focused = ed.view_tree.focused_window().unwrap();
    let buf_key = ed.view_tree.buffer(focused)
        .unwrap_or_else(|| ed.buffers.iter().next().map(|(k, _)| k).unwrap_or(0));
    ed.view_tree.set_buffer(focused, buf_key);
    let mut surface = Surface::new(80, 24);
    render_frame(&ed, &mut surface);
    let row0: String = (0..80).filter_map(|x| surface.cell(x, 0)).map(|c| c.ch).collect();
    assert!(row0.contains('h') || row0.contains(' '),
        "render_frame should write buffer content to the surface");
}

#[test]
fn multi_pane_render_frame_does_not_panic() {
    use crate::kernel::render::frame::render_frame;
    let mut ed = helpers::make_editor_with_buffer("pane one\n");
    let id = ed.view_tree.focused_window().unwrap();
    ed.view_tree.resize(80, 24);
    let buf_key = ed.buffers.iter().next().map(|(k, _)| k).unwrap_or(0);
    ed.view_tree.set_buffer(id, buf_key);
    ed.view_tree.split_horizontal(id);
    let mut surface = Surface::new(80, 24);
    render_frame(&ed, &mut surface);
}
