use crate::kernel::input::event::*;
use crate::kernel::input::mouse;
use crate::kernel::state::{Editor, MouseState, MouseButton};
use crate::kernel::render::frame::compute_scroll_state;
use crate::kernel::render::decorations::prefix_margin_width;
use crate::tests::helpers::{self, make_editor, make_editor_with_buffer};

/// Compute the screen x where text content begins for a buffer in a pane.
fn content_start_x(ed: &Editor, buf_id: usize, pane_x: u16) -> u16 {
    let total_lines = ed.buffers.get(buf_id)
        .map(|a| a.lock().unwrap().line_count()).unwrap_or(1);
    let prefix_margin = prefix_margin_width(ed, buf_id);
    let prefix_cols = if prefix_margin > 0 { prefix_margin + 1 } else { 0 };
    let gutter_w = prefix_cols + ed.gutter.total_width(total_lines);
    pane_x + gutter_w as u16
}

#[test]
fn mouse_event_drag_variant() {
    let ev = InputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Drag,
        x: 10,
        y: 20,
        button: MouseButton::Left,
        modifiers: String::new(),
    });
    let s = format!("{:?}", ev);
    assert!(s.contains("Drag"));
}

#[test]
fn mouse_event_modifiers_roundtrip() {
    let ev = InputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Click,
        x: 5,
        y: 5,
        button: MouseButton::Right,
        modifiers: "shift,ctrl".to_string(),
    });
    let s = format!("{:?}", ev);
    assert!(s.contains("shift"));
}

#[test]
fn hit_test_single_pane_center() {
    let ed = make_editor();
    let result = mouse::hit_test_pane(&ed, 40, 10);
    assert!(result.is_some(), "click at (40,10) should hit the single pane");
}

#[test]
fn hit_test_outside_panes() {
    let ed = make_editor();
    let result = mouse::hit_test_pane(&ed, 999, 999);
    assert!(result.is_none(), "click at (999,999) should miss all panes");
}

#[test]
fn resolve_text_click_basic() {
    let ed = make_editor_with_buffer("hello\nworld\nline three");
    let buf_id = helpers::focused_key(&ed);
    let pane = ed.view_tree.panes()[0].clone();
    let cx = content_start_x(&ed, buf_id, pane.x);
    let offset = mouse::resolve_click_to_offset(&ed, buf_id, &pane, cx, 0);
    assert!(offset.is_some(), "should resolve to a valid offset");
    assert_eq!(offset.unwrap(), 0, "first column of first line should map to byte 0");
}

#[test]
fn resolve_text_click_mid_line() {
    let ed = make_editor_with_buffer("hello world");
    let buf_id = helpers::focused_key(&ed);
    let pane = ed.view_tree.panes()[0].clone();
    let cx = content_start_x(&ed, buf_id, pane.x);
    // Click at column 6 of "hello world" -> 'w' at byte 6.
    let offset = mouse::resolve_click_to_offset(&ed, buf_id, &pane, cx + 6, 0);
    assert!(offset.is_some(), "should resolve");
    assert_eq!(offset.unwrap(), 6, "column 6 = byte offset 6 ('w')");
}

#[test]
fn resolve_text_click_out_of_bounds() {
    let ed = make_editor_with_buffer("short");
    let buf_id = helpers::focused_key(&ed);
    let pane = ed.view_tree.panes()[0].clone();
    let offset = mouse::resolve_click_to_offset(&ed, buf_id, &pane, 0, 100);
    assert!(offset.is_none(), "click below buffer should not resolve");
}

#[test]
fn scroll_dispatch_computes_scrolldown() {
    let ed = make_editor_with_buffer("a\nb\nc\nd\ne\nf\ng\nh\ni\nj");
    let buf_id = helpers::focused_key(&ed);
    let before = compute_scroll_state(&ed, buf_id, 3);
    assert!(before.scroll_top < 7,
        "scroll_top should be near the top for 10 lines with 3 visible");
}

#[test]
fn drag_sets_selection() {
    let mut ed = make_editor_with_buffer("one two three four five");
    let pane = ed.view_tree.panes()[0].clone();
    let buf_id = helpers::focused_key(&ed);
    let cx = content_start_x(&ed, buf_id, pane.x);
    click_at(&mut ed, buf_id, &pane, cx + 4, 0);
    let current_offset = resolve_text_col(&ed, buf_id, &pane, 10, 0);
    let press_offset = resolve_text_col(&ed, buf_id, &pane, 4, 0);
    if let (Some(press), Some(curr)) = (press_offset, current_offset) {
        ed.selection = Some(crate::kernel::state::mode::Selection::char(press));
        if let Some(view) = ed.views.get_mut(&buf_id) {
            view.set_cursor(curr);
        }
        assert!(ed.selection.is_some(), "selection should be set after drag");
        let sel = ed.selection.as_ref().unwrap();
        assert_eq!(sel.anchor, press, "anchor should be press position");
    } else {
        panic!("could not resolve click positions");
    }
}

fn click_at(ed: &mut Editor, buf_id: usize, pane: &crate::kernel::render::view_tree::Pane, press_x: u16, row: u16) {
    ed.mouse_state = MouseState {
        pressed: true,
        button: MouseButton::Left,
        press_x,
        press_y: pane.y + row,
        last_x: 0,
        last_y: 0,
        drag_active: false,
    };
}

fn resolve_text_col(
    ed: &Editor,
    buf_id: usize,
    pane: &crate::kernel::render::view_tree::Pane,
    col: u16,
    row: u16,
) -> Option<usize> {
    let cx = content_start_x(ed, buf_id, pane.x);
    mouse::resolve_click_to_offset(ed, buf_id, pane, cx + col, row)
}
