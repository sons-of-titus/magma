//! Viewport scrolling — sticky cursor-follow, wheel pinning, and click
//! mapping against the scrolled pane.

use crate::kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
use crate::kernel::input::mouse;
use crate::kernel::render::frame::{
    gutter_cols, pane_scroll_left, pane_scroll_top, render_frame, sync_viewport_scroll,
};
use crate::kernel::render::surface::Surface;
use crate::kernel::render::view_tree::Pane;
use crate::kernel::state::Editor;
use crate::tests::helpers::{focused_key, make_editor_with_buffer};

fn set_pane_height(ed: &mut Editor, h: u16) {
    ed.view_tree.panes_mut()[0].height = h;
}

fn pane_of(ed: &Editor) -> Pane {
    ed.view_tree.panes()[0].clone()
}

fn cursor_line_of(ed: &Editor, buf_id: usize) -> usize {
    let offset = ed.view_of_buffer(buf_id).unwrap().cursor.offset;
    let arc = ed.buffers.get(buf_id).unwrap();
    let buf = arc.lock().unwrap();
    buf.position_of(offset).line
}

fn set_cursor_line(ed: &mut Editor, buf_id: usize, line: usize) {
    let start = {
        let arc = ed.buffers.get(buf_id).unwrap();
        let buf = arc.lock().unwrap();
        buf.line_start_offset(line).unwrap()
    };
    if let Some(view) = ed.view_of_buffer_mut(buf_id) {
        view.set_cursor(start);
    }
}

#[test]
fn cursor_move_inside_view_keeps_viewport_still() {
    let mut ed = make_editor_with_buffer("line\n".repeat(100).trim_end());
    set_pane_height(&mut ed, 11);
    let buf_id = focused_key(&ed);

    sync_viewport_scroll(&mut ed);
    let pane = pane_of(&ed);
    assert_eq!(pane_scroll_top(&ed, &pane, buf_id, 10), 0);

    set_cursor_line(&mut ed, buf_id, 5);
    sync_viewport_scroll(&mut ed);
    assert_eq!(cursor_line_of(&ed, buf_id), 5);
    let pane = pane_of(&ed);
    assert_eq!(pane_scroll_top(&ed, &pane, buf_id, 10), 0,
        "cursor moving within the viewport must not scroll the text");
}

#[test]
fn cursor_below_view_snaps_minimally() {
    let mut ed = make_editor_with_buffer("line\n".repeat(100).trim_end());
    set_pane_height(&mut ed, 11);
    let buf_id = focused_key(&ed);

    // Jumping into an unsynced pane puts the target line at the top
    // (vim `:{N}` semantics).
    set_cursor_line(&mut ed, buf_id, 50);
    sync_viewport_scroll(&mut ed);
    let pane = pane_of(&ed);
    assert_eq!(pane_scroll_top(&ed, &pane, buf_id, 10), 50);

    // Steps within the view leave it alone; crossing the bottom edge
    // scrolls exactly as far as the cursor needs.
    set_cursor_line(&mut ed, buf_id, 52);
    sync_viewport_scroll(&mut ed);
    let pane = pane_of(&ed);
    assert_eq!(pane_scroll_top(&ed, &pane, buf_id, 10), 50);

    set_cursor_line(&mut ed, buf_id, 65);
    sync_viewport_scroll(&mut ed);
    let pane = pane_of(&ed);
    assert_eq!(pane_scroll_top(&ed, &pane, buf_id, 10), 56,
        "one row past the bottom edge scrolls the minimum amount");
}

#[test]
fn cursor_above_view_snaps_up() {
    let mut ed = make_editor_with_buffer("line\n".repeat(100).trim_end());
    set_pane_height(&mut ed, 11);
    let buf_id = focused_key(&ed);

    set_cursor_line(&mut ed, buf_id, 50);
    sync_viewport_scroll(&mut ed);
    set_cursor_line(&mut ed, buf_id, 20);
    sync_viewport_scroll(&mut ed);
    let pane = pane_of(&ed);
    assert_eq!(pane_scroll_top(&ed, &pane, buf_id, 10), 20,
        "cursor above the viewport pulls the top line up to the cursor");
}

#[test]
fn wheel_scrolls_viewport_without_moving_cursor() {
    let mut ed = make_editor_with_buffer("line\n".repeat(100).trim_end());
    set_pane_height(&mut ed, 11);
    ed.options.insert("mouse-support".to_string(), "true".to_string());
    let buf_id = focused_key(&ed);
    let pane = pane_of(&ed);
    let surface = Surface::new(pane.width, pane.height);

    let wheel = MouseEvent {
        kind: MouseEventKind::Scroll(3),
        x: pane.x + 20,
        y: pane.y + 5,
        button: MouseButton::Left,
        modifiers: String::new(),
    };
    mouse::dispatch_mouse(&mut ed, &surface, &wheel);

    assert_eq!(cursor_line_of(&ed, buf_id), 0, "wheel must not move the cursor");
    let pane = pane_of(&ed);
    assert_eq!(pane.scroll_offset, 0, "scrolling down from the top clamps at 0");

    let wheel_up = MouseEvent { kind: MouseEventKind::Scroll(-5), ..wheel };
    mouse::dispatch_mouse(&mut ed, &surface, &wheel_up);
    let pane = pane_of(&ed);
    assert_eq!(pane.scroll_offset, 5, "wheel up reveals earlier lines");
    assert!(pane.scroll_pinned);
    assert_eq!(cursor_line_of(&ed, buf_id), 0);
}

#[test]
fn cursor_move_releases_wheel_pin() {
    let mut ed = make_editor_with_buffer("line\n".repeat(100).trim_end());
    set_pane_height(&mut ed, 11);
    ed.options.insert("mouse-support".to_string(), "true".to_string());
    let buf_id = focused_key(&ed);
    let pane = pane_of(&ed);
    let surface = Surface::new(pane.width, pane.height);

    let wheel_up = MouseEvent {
        kind: MouseEventKind::Scroll(-40),
        x: pane.x + 20,
        y: pane.y + 5,
        button: MouseButton::Left,
        modifiers: String::new(),
    };
    mouse::dispatch_mouse(&mut ed, &surface, &wheel_up);

    // Typing / motion moves the cursor: the pin must release so the view
    // follows again.
    set_cursor_line(&mut ed, buf_id, 3);
    sync_viewport_scroll(&mut ed);
    let pane = pane_of(&ed);
    assert!(!pane.scroll_pinned);
    assert_eq!(pane_scroll_top(&ed, &pane, buf_id, 10), 3);
}

#[test]
fn click_maps_against_the_scrolled_viewport() {
    let mut ed = make_editor_with_buffer("line\n".repeat(100).trim_end());
    set_pane_height(&mut ed, 11);
    ed.options.insert("mouse-support".to_string(), "true".to_string());
    let buf_id = focused_key(&ed);
    let pane = pane_of(&ed);
    let surface = Surface::new(pane.width, pane.height);

    let wheel_up = MouseEvent {
        kind: MouseEventKind::Scroll(-10),
        x: pane.x + 20,
        y: pane.y + 5,
        button: MouseButton::Left,
        modifiers: String::new(),
    };
    mouse::dispatch_mouse(&mut ed, &surface, &wheel_up);

    let pane = pane_of(&ed);
    let top = pane_scroll_top(&ed, &pane, buf_id, 10);
    assert_eq!(top, 10);
    let offset = mouse::resolve_click_to_offset(&ed, buf_id, &pane, 50, pane.y + 2)
        .expect("click inside the viewport resolves");
    let arc = ed.buffers.get(buf_id).unwrap();
    let buf = arc.lock().unwrap();
    assert_eq!(buf.position_of(offset).line, 12,
        "click on viewport row 2 with top 10 must hit buffer line 12");
}

#[test]
fn buffer_switch_rederives_viewport() {
    let mut ed = make_editor_with_buffer("line\n".repeat(100).trim_end());
    set_pane_height(&mut ed, 11);
    let buf_a = focused_key(&ed);

    set_cursor_line(&mut ed, buf_a, 50);
    sync_viewport_scroll(&mut ed);

    let other = ed.create_buffer_from_str("other", "alpha\nbeta\n");
    ed.view_tree.set_buffer(ed.view_tree.focused_window().unwrap(), other);
    sync_viewport_scroll(&mut ed);

    let pane = pane_of(&ed);
    assert_eq!(pane.scroll_buf, Some(other));
    assert_eq!(pane_scroll_top(&ed, &pane, other, 10), 0,
        "a fresh buffer starts at its own cursor, not the old scroll offset");
}

#[test]
fn fold_collapsed_rows_map_clicks_to_visible_lines() {
    let mut ed = make_editor_with_buffer("l0\nl1\nl2\nl3\nl4\nl5\nl6\n");
    set_pane_height(&mut ed, 11);
    let buf_id = focused_key(&ed);

    // Fold lines 2..4 (0-based): the fold header (line 2) occupies one row,
    // lines 3–4 disappear from the viewport.
    let l2 = ed.buffers.get(buf_id).unwrap().lock().unwrap()
        .line_start_offset(2).unwrap();
    let l5 = ed.buffers.get(buf_id).unwrap().lock().unwrap()
        .line_start_offset(5).unwrap();
    if let Some(view) = ed.view_of_buffer_mut(buf_id) {
        view.add_fold(l2, l5);
    }

    sync_viewport_scroll(&mut ed);
    let _pane = pane_of(&ed);
    // Row 0 → line 0, row 1 → line 1, row 2 → fold header line 2,
    // row 3 → line 5 (first line after the fold).
    assert_eq!(mouse::visible_row_to_abs_line(&ed, buf_id, 0, 1, 10), Some(1));
    assert_eq!(mouse::visible_row_to_abs_line(&ed, buf_id, 0, 2, 10), Some(2));
    assert_eq!(mouse::visible_row_to_abs_line(&ed, buf_id, 0, 3, 10), Some(5));
}

// ── GUI selection idioms ─────────────────────────────────────────────────────

use std::time::Duration;

fn click(ed: &mut Editor, surface: &crate::kernel::render::surface::Surface, x: u16, y: u16, gap_ms: u64) {
    std::thread::sleep(Duration::from_millis(gap_ms));
    let ev = MouseEvent {
        kind: MouseEventKind::Click,
        x, y,
        button: MouseButton::Left,
        modifiers: String::new(),
    };
    mouse::dispatch_mouse(ed, surface, &ev);
}

#[test]
fn double_click_selects_word_under_pointer() {
    let mut ed = make_editor_with_buffer("alpha beta gamma\n");
    ed.options.insert("mouse-support".to_string(), "true".to_string());
    let buf_id = focused_key(&ed);
    let pane = pane_of(&ed);
    let surface = Surface::new(pane.width, pane.height);
    sync_viewport_scroll(&mut ed);
    // No gutter providers in this bare editor: cell x == text column.
    let beta_col = 8u16; // inside "beta"

    click(&mut ed, &surface, beta_col, pane.y, 400); // single: plain cursor
    assert!(ed.selection.is_none());
    click(&mut ed, &surface, beta_col, pane.y, 0);   // double on "beta"
    let sel = ed.selection.expect("double click selects");
    let arc = ed.buffers.get(buf_id).unwrap();
    let buf = arc.lock().unwrap();
    assert_eq!(&buf.slice(0, buf.len())[sel.anchor..], "beta gamma\n");
}

#[test]
fn triple_click_selects_the_whole_line() {
    let mut ed = make_editor_with_buffer("one\ntwo\n");
    ed.options.insert("mouse-support".to_string(), "true".to_string());
    let buf_id = focused_key(&ed);
    let pane = pane_of(&ed);
    let surface = Surface::new(pane.width, pane.height);
    sync_viewport_scroll(&mut ed);

    click(&mut ed, &surface, 20, pane.y + 1, 400);
    click(&mut ed, &surface, 20, pane.y + 1, 0);
    click(&mut ed, &surface, 20, pane.y + 1, 0);
    let sel = ed.selection.expect("triple click selects the line");
    let arc = ed.buffers.get(buf_id).unwrap();
    let buf = arc.lock().unwrap();
    assert_eq!(&buf.slice(0, buf.len())[sel.anchor..], "two\n");
}

#[test]
fn right_click_does_not_move_the_cursor() {
    let mut ed = make_editor_with_buffer("hello\n");
    ed.options.insert("mouse-support".to_string(), "true".to_string());
    let buf_id = focused_key(&ed);
    let pane = pane_of(&ed);
    let surface = Surface::new(pane.width, pane.height);

    let ev = MouseEvent {
        kind: MouseEventKind::Click,
        x: 30,
        y: pane.y,
        button: MouseButton::Right,
        modifiers: String::new(),
    };
    mouse::dispatch_mouse(&mut ed, &surface, &ev);
    let offset = ed.view_of_buffer(buf_id).unwrap().cursor_offset();
    assert_eq!(offset, 0, "secondary click is reserved for the context menu");
}

#[test]
fn word_range_at_scopes_to_the_word_run() {
    let ed = make_editor_with_buffer("fn snake_case_name():\n");
    let buf_id = focused_key(&ed);
    let arc = ed.buffers.get(buf_id).unwrap();
    let buf = arc.lock().unwrap();
    // Offset inside "snake_case_name" (after "fn ").
    let mid = 3 + 7;
    let (s, e) = mouse::word_range_at(&buf, mid).expect("inside a word");
    assert_eq!(buf.slice(s, e), "snake_case_name");
    // Right after a word (on the following space): selects that word.
    let (s2, e2) = mouse::word_range_at(&buf, 2).expect("adjacent word still selects");
    assert_eq!(buf.slice(s2, e2), "fn");
}

// ── Horizontal scrolling ────────────────────────────────────────────────────

fn content_cols_of(ed: &Editor, buf_id: usize) -> usize {
    let total = ed.buffers.get(buf_id).unwrap().lock().unwrap().line_count();
    (pane_of(ed).width as usize).saturating_sub(gutter_cols(ed, buf_id, total))
}

fn set_cursor_offset(ed: &mut Editor, buf_id: usize, offset: usize) {
    if let Some(view) = ed.view_of_buffer_mut(buf_id) {
        view.set_cursor(offset);
    }
}

#[test]
fn caret_past_right_edge_pans_the_view() {
    let mut ed = make_editor_with_buffer(&"x".repeat(200));
    ed.view_tree.panes_mut()[0].width = 20;
    let buf_id = focused_key(&ed);
    set_cursor_offset(&mut ed, buf_id, 100);

    sync_viewport_scroll(&mut ed);
    let pane = pane_of(&ed);
    let left = pane_scroll_left(&pane, buf_id);
    let content = content_cols_of(&ed, buf_id);
    assert!(left > 0, "a caret off the right edge must pan the view");
    assert!(left <= 100, "the pan is minimal, never past the caret");
    assert!(100 - left < content, "the caret must be inside the panned view");
}

#[test]
fn caret_back_left_pans_back_to_the_start() {
    let mut ed = make_editor_with_buffer(&"x".repeat(200));
    ed.view_tree.panes_mut()[0].width = 20;
    let buf_id = focused_key(&ed);
    set_cursor_offset(&mut ed, buf_id, 100);
    sync_viewport_scroll(&mut ed);
    assert!(pane_scroll_left(&pane_of(&ed), buf_id) > 0);

    set_cursor_offset(&mut ed, buf_id, 0);
    sync_viewport_scroll(&mut ed);
    assert_eq!(pane_scroll_left(&pane_of(&ed), buf_id), 0,
        "the caret returning to column 0 scrolls the view back");
}

#[test]
fn horizontal_scroll_resets_on_buffer_switch() {
    let mut ed = make_editor_with_buffer(&"x".repeat(200));
    ed.view_tree.panes_mut()[0].width = 20;
    let buf_id = focused_key(&ed);
    {
        let pane = &mut ed.view_tree.panes_mut()[0];
        pane.scroll_left = 12;
        pane.scroll_left_pinned = true;
        pane.scroll_buf = Some(buf_id + 999); // a different buffer's pin
    }
    sync_viewport_scroll(&mut ed);
    let pane = pane_of(&ed);
    assert_eq!(pane.scroll_left, 0, "a buffer switch re-derives the pan");
    assert_eq!(pane.scroll_buf, Some(buf_id));
}

#[test]
fn horizontal_wheel_pans_and_pins() {
    let mut ed = make_editor_with_buffer(&"x".repeat(200));
    ed.view_tree.panes_mut()[0].width = 20;
    ed.options.insert("mouse-support".to_string(), "true".to_string());
    let pane = pane_of(&ed);
    let surface = Surface::new(pane.width, pane.height);

    let wheel = MouseEvent {
        kind: MouseEventKind::ScrollX(-5),
        x: pane.x + 5,
        y: pane.y + 2,
        button: MouseButton::Left,
        modifiers: String::new(),
    };
    mouse::dispatch_mouse(&mut ed, &surface, &wheel);
    let pane = pane_of(&ed);
    assert_eq!(pane.scroll_left, 5, "wheel pan moves the first visible column");
    assert!(pane.scroll_left_pinned);

    let back = MouseEvent { kind: MouseEventKind::ScrollX(50), ..wheel };
    mouse::dispatch_mouse(&mut ed, &surface, &back);
    assert_eq!(pane_of(&ed).scroll_left, 0, "panning back clamps at column 0");
}

#[test]
fn click_maps_through_the_horizontal_scroll() {
    let mut ed = make_editor_with_buffer(&"x".repeat(200));
    ed.view_tree.panes_mut()[0].width = 40;
    let buf_id = focused_key(&ed);
    let total = ed.buffers.get(buf_id).unwrap().lock().unwrap().line_count();
    let gutter = gutter_cols(&ed, buf_id, total) as u16;
    {
        let pane = &mut ed.view_tree.panes_mut()[0];
        pane.scroll_left = 10;
        pane.scroll_left_pinned = true;
        pane.scroll_buf = Some(buf_id);
    }
    let pane = pane_of(&ed);
    // Five cells into the visible text, which is column 15 after the pan.
    let off = mouse::resolve_click_to_offset(&ed, buf_id, &pane, pane.x + gutter + 5, pane.y);
    assert_eq!(off, Some(15), "a click maps to scroll_left + clicked column");
}

#[test]
fn render_shifts_content_by_the_horizontal_scroll() {
    let mut ed = make_editor_with_buffer("0123456789ABCDEFGHIJ");
    ed.view_tree.panes_mut()[0].width = 12;
    let buf_id = focused_key(&ed);
    {
        let pane = &mut ed.view_tree.panes_mut()[0];
        pane.scroll_left = 10;
        pane.scroll_left_pinned = true;
        pane.scroll_buf = Some(buf_id);
    }
    let pane = pane_of(&ed);
    let total = ed.buffers.get(buf_id).unwrap().lock().unwrap().line_count();
    let gutter = gutter_cols(&ed, buf_id, total) as u16;
    let mut surface = Surface::new(pane.width, pane.height);
    render_frame(&ed, &mut surface);

    let cell = surface.cell(gutter, pane.y).expect("first content cell");
    assert_eq!(cell.ch, 'A',
        "after panning 10 columns, 'A' (column 10) is the first visible glyph");
}
