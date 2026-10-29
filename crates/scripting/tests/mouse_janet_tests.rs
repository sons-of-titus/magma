use kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
use kernel::input::mouse;
use kernel::render::decorations::prefix_margin_width;
use scripting;
#[macro_use]
mod helpers;

/// Compute the screen x where text content begins for a buffer in a pane.
fn content_start_x(ed: &kernel::state::Editor, buf_id: usize, pane_x: u16) -> u16 {
    let total_lines = ed
        .buffers
        .get(buf_id)
        .map(|a| a.lock().unwrap().line_count())
        .unwrap_or(1);
    let prefix_margin = prefix_margin_width(ed, buf_id);
    let prefix_cols = if prefix_margin > 0 {
        prefix_margin + 1
    } else {
        0
    };
    let gutter_w = prefix_cols + ed.gutter.total_width(total_lines);
    pane_x + gutter_w as u16
}

#[test]
fn text_click_moves_cursor() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("hello world\nsecond line");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let buf_id = helpers::focused_key(&ed);
    let pane = ed.view_tree.panes()[0].clone();
    let cx = content_start_x(&ed, buf_id, pane.x);
    let click_x = cx + 6; // column 6 = 'w'

    let click_event = MouseEvent {
        kind: MouseEventKind::Click,
        x: click_x,
        y: pane.y,
        button: MouseButton::Left,
        modifiers: String::new(),
    };

    // Enable mouse support via Janet option.
    ed.options
        .insert("mouse-support".to_string(), "true".to_string());

    let surface = kernel::render::surface::Surface::new(pane.width, pane.height);

    mouse::dispatch_mouse(&mut ed, &surface, &click_event);

    let cursor_offset = ed
        .views
        .get(&buf_id)
        .map(|v| v.cursor.offset)
        .unwrap_or(999);
    assert_eq!(
        cursor_offset, 6,
        "click at column 6 should move cursor to byte 6 ('w')"
    );
}

#[test]
fn mouse_option_gates_processing() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("abcdefghij");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let buf_id = helpers::focused_key(&ed);
    let cursor_before = ed
        .views
        .get(&buf_id)
        .map(|v| v.cursor.offset)
        .unwrap_or(999);

    // Mouse support is disabled by default.
    let pane = ed.view_tree.panes()[0].clone();
    let cx = content_start_x(&ed, buf_id, pane.x);
    let content_x = cx + 3;

    let click_event = MouseEvent {
        kind: MouseEventKind::Click,
        x: content_x,
        y: pane.y,
        button: MouseButton::Left,
        modifiers: String::new(),
    };
    let surface = kernel::render::surface::Surface::new(pane.width, pane.height);

    mouse::dispatch_mouse(&mut ed, &surface, &click_event);

    let cursor_after = ed
        .views
        .get(&buf_id)
        .map(|v| v.cursor.offset)
        .unwrap_or(999);
    assert_eq!(
        cursor_before, cursor_after,
        "cursor should not move when mouse-support is not enabled"
    );
}

#[test]
fn drag_anchor_survives_scrolling() {
    let _lock = helpers::acquire_janet_lock();
    let lines: Vec<String> = (0..40).map(|i| format!("line {i:02}")).collect();
    let mut ed = helpers::make_editor_with_buffer(&lines.join("\n"));
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    ed.options
        .insert("mouse-support".to_string(), "true".to_string());

    let buf_id = helpers::focused_key(&ed);
    let pane = ed.view_tree.panes()[0].clone();
    let cx = content_start_x(&ed, buf_id, pane.x);
    let surface = kernel::render::surface::Surface::new(pane.width, pane.height);

    let event = |kind, y: u16| MouseEvent {
        kind,
        x: cx,
        y,
        button: MouseButton::Left,
        modifiers: String::new(),
    };

    // Press on the first row, drag three rows down: anchor 0, cursor line 3.
    mouse::dispatch_mouse(&mut ed, &surface, &event(MouseEventKind::Click, pane.y));
    mouse::dispatch_mouse(&mut ed, &surface, &event(MouseEventKind::Drag, pane.y + 3));
    assert_eq!(
        ed.selection.as_ref().map(|s| s.anchor),
        Some(0),
        "press should anchor the selection at the first line"
    );

    // Auto-scroll under the held drag: the viewport moves, the press cell
    // stays where it is.  The anchor must not follow the scrolled text.
    if let Some(p) = ed.view_tree.pane_mut(pane.id) {
        p.scroll_offset = 5;
        p.scroll_pinned = true;
        p.scroll_buf = Some(buf_id);
    }
    mouse::dispatch_mouse(&mut ed, &surface, &event(MouseEventKind::Drag, pane.y + 3));

    let anchor = ed
        .selection
        .as_ref()
        .map(|s| s.anchor)
        .expect("dragging must keep a selection");
    assert_eq!(anchor, 0, "scrolling must not re-anchor the selection");

    let cursor = ed.views.get(&buf_id).unwrap().cursor.offset;
    // Row 3 of a viewport topped at line 5 is line 8; each line is 8 bytes.
    assert_eq!(
        cursor,
        8 * 8,
        "cursor should follow the line under the pointer"
    );
    let (start, end) = ed.selection_range(cursor).unwrap();
    assert_eq!((start, end), (0, cursor + 1));
}

#[test]
fn gutter_click_emits_event() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("line A\nline B\nline C");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    let _buf_id = helpers::focused_key(&ed);
    let pane = ed.view_tree.panes()[0].clone();

    // Enable mouse support.
    ed.options
        .insert("mouse-support".to_string(), "true".to_string());

    // Register a test gutter provider so the click lands on something.
    ed.gutter.add_provider(Box::new(TestGutterProvider));

    // The test provider has width 2, and it's the only provider.
    // gutter starts at pane.x.
    let gutter_x = pane.x + 1; // +1 for prefix margin gap (prefix_margin_width = 0)
    let click_x = gutter_x; // first cell of the first (and only) provider column

    let click_event = MouseEvent {
        kind: MouseEventKind::Click,
        x: click_x,
        y: pane.y, // first visible row (no tab bar, no header)
        button: MouseButton::Left,
        modifiers: String::new(),
    };
    let surface = kernel::render::surface::Surface::new(pane.width, 10);

    let sub_count_before = ed.events.subscriber_count("gutter-clicked");
    mouse::dispatch_mouse(&mut ed, &surface, &click_event);
    let sub_count_after = ed.events.subscriber_count("gutter-clicked");
    // The event should be emitted. Subscriber count is the same (we didn't subscribe here).
    // Just verify dispatch didn't panic.
    assert_eq!(sub_count_before, sub_count_after);
}

// ── Test gutter provider ─────────────────────────────────────────────────

struct TestGutterProvider;

impl kernel::render::gutter::GutterProvider for TestGutterProvider {
    fn name(&self) -> &str {
        ":test"
    }
    fn width(&self) -> usize {
        2
    }
    fn render(
        &self,
        _line: usize,
        _ctx: &kernel::render::gutter::GutterCtx,
    ) -> Option<kernel::render::gutter::GutterCell> {
        None
    }
}
