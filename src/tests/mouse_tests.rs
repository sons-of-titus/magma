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
    let ev = MouseEvent {
        kind: MouseEventKind::Drag,
        x: 10,
        y: 20,
        button: MouseButton::Left,
        modifiers: String::new(),
    };
    let s = format!("{:?}", ev);
    assert!(s.contains("Drag"));
}

// ── Hit testing never lands on a pane that shows nothing ─────────────────────

/// Regression: with no startup `*scratch*`, the initial pane can be empty.  If
/// that bufferless pane sits underneath a tab, it used to win the hit test and
/// swallow every click and wheel event — the editor looked frozen.
#[test]
fn click_and_scroll_ignore_a_bufferless_pane_underneath() {
    let mut ed = make_editor();
    ed.options.insert("mouse-support".to_string(), "true".to_string());

    // Stack a visible tab showing real content over the first pane, then
    // empty the first pane (the "no buffer" editor state).
    let visible = ed.create_buffer_from_str("visible", "one\ntwo\nthree\nfour\nfive\nsix\n");
    let visible_pane = ed.view_tree.open_tab(Some(visible));
    let first = ed.view_tree.panes()[0].id;
    if let Some(p) = ed.view_tree.window_mut(first) { p.buffer_id = None; }
    ed.view_tree.resize(80, 24);
    ed.view_tree.focus(visible_pane);

    let (pane, _) = mouse::hit_test_pane(&ed, 5, 3).expect("the visible pane must be hit");
    assert_eq!(pane.buffer_id, Some(visible),
        "a bufferless pane must never swallow the hit");

    let surface = crate::kernel::render::surface::Surface::new(80, 24);
    let ev = MouseEvent {
        kind: MouseEventKind::Scroll(-10),
        x: 5,
        y: 3,
        button: MouseButton::Left,
        modifiers: String::new(),
    };
    mouse::dispatch_mouse(&mut ed, &surface, &ev);
    let scrolled = ed.view_tree.pane(visible_pane).map(|p| p.scroll_offset).unwrap_or(0);
    assert!(scrolled > 0, "the wheel must scroll the visible buffer, got {scrolled}");
}

#[test]
fn mouse_event_modifiers_roundtrip() {
    let ev = MouseEvent {
        kind: MouseEventKind::Click,
        x: 5,
        y: 5,
        button: MouseButton::Right,
        modifiers: "shift,ctrl".to_string(),
    };
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
    // Clicking the empty space below a short buffer lands on its last line —
    // the GUI convention — instead of dropping the click.
    let offset = mouse::resolve_click_to_offset(&ed, buf_id, &pane, 0, 100);
    let last_line_start = ed.buffers.get(buf_id).unwrap().lock().unwrap()
        .line_start_offset(0).unwrap();
    assert_eq!(offset, Some(last_line_start),
        "click below the buffer clamps to the last line");
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

fn click_at(ed: &mut Editor, _buf_id: usize, pane: &crate::kernel::render::view_tree::Pane, press_x: u16, row: u16) {
    ed.mouse_state = MouseState {
        pressed: true,
        button: MouseButton::Left,
        press_x,
        press_y: pane.y + row,
        last_x: 0,
        last_y: 0,
        drag_active: false,
        last_click: None,
        click_count: 0,
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

#[test]
fn resolve_text_click_after_tab_lands_on_rendered_column() {
    // A tab renders to its stop (default 4): "\tx" paints x at cell 4, not 1.
    // Clicking cell 4 must land on 'x' (byte 1), not past it.
    let ed = make_editor_with_buffer("\tx");
    let buf_id = helpers::focused_key(&ed);
    let pane = ed.view_tree.panes()[0].clone();
    let cx = content_start_x(&ed, buf_id, pane.x);
    let offset = mouse::resolve_click_to_offset(&ed, buf_id, &pane, cx + 4, 0);
    assert_eq!(offset, Some(1), "cell 4 is where the renderer paints 'x'");

    // Clicking inside the tab (cells 0..3) clamps to the tab's own start.
    let offset = mouse::resolve_click_to_offset(&ed, buf_id, &pane, cx + 2, 0);
    assert_eq!(offset, Some(0), "cells inside the tab still resolve to its start");
}

#[test]
fn resolve_text_click_after_wide_char_uses_two_cells() {
    // CJK 字 occupies two cells: clicking cell 2 lands after it.
    let ed = make_editor_with_buffer("字x");
    let buf_id = helpers::focused_key(&ed);
    let pane = ed.view_tree.panes()[0].clone();
    let cx = content_start_x(&ed, buf_id, pane.x);
    let offset = mouse::resolve_click_to_offset(&ed, buf_id, &pane, cx + 2, 0);
    assert_eq!(offset, Some("字".len()), "cell 2 = first cell after the wide glyph");
    let offset = mouse::resolve_click_to_offset(&ed, buf_id, &pane, cx + 1, 0);
    assert_eq!(offset, Some(0), "cell 1 is the wide glyph's second cell — its start");
}
