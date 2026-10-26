//! Regression tests for `RenderHost` — the read-only capability surface the
//! frame layer uses instead of the concrete `Editor`.
//!
//! The stub below is deliberately not an `Editor`: it owns six pieces of data
//! and nothing else.  If the frame helpers ever reach for editor state beyond
//! the trait, these tests stop compiling — which is the point of the seam.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::kernel::render::decorations::render_overlays;
use crate::kernel::render::editor_view::{build_visible_rows, VisibleRow};
use crate::kernel::render::frame::{
    bool_option, compute_scroll_state, frame_panes, gutter_cols, horizontal_extent, int_option,
    pane_scroll_top, prepare_frame, sync_viewport_scroll,
};
use crate::kernel::render::host::{FrameHost, PaneMetrics, RenderHost, ViewportHost};
use crate::kernel::state::Overlay;
use crate::kernel::render::surface::Surface;
use crate::kernel::render::view_tree::Pane;
use crate::kernel::state::id::{BufferId, WindowId};
use crate::kernel::text_engine::{Buffer, BufferView, Position};

struct StubHost {
    options: HashMap<String, String>,
    buffers: slab::Slab<Arc<Mutex<Buffer>>>,
    views: HashMap<usize, BufferView>,
    glyphs: HashMap<char, u8>,
    gutter: usize,
    panes: Vec<Pane>,
    focused: Option<WindowId>,
    pane_metrics: Vec<Option<PaneMetrics>>,
    theme: HashMap<String, (u8, u8, u8)>,
}

impl StubHost {
    fn with_text(text: &str) -> Self {
        let mut buffers = slab::Slab::new();
        let buffer = Arc::new(Mutex::new(Buffer::from_string(BufferId(0), "stub", text)));
        let key = buffers.insert(buffer.clone());
        let mut views = HashMap::new();
        views.insert(key, BufferView::new(buffer));
        Self {
            options: HashMap::new(),
            buffers,
            views,
            glyphs: HashMap::new(),
            gutter: 0,
            panes: Vec::new(),
            focused: None,
            pane_metrics: Vec::new(),
            theme: HashMap::new(),
        }
    }

    fn set_cursor(&mut self, key: usize, offset: usize) {
        if let Some(view) = self.views.get_mut(&key) {
            view.cursor = Position { line: 0, column: 0, offset };
        }
    }
}

impl RenderHost for StubHost {
    fn option(&self, name: &str) -> Option<&str> {
        self.options.get(name).map(String::as_str)
    }

    fn buffer(&self, key: usize) -> Option<&Arc<Mutex<Buffer>>> {
        self.buffers.get(key)
    }

    fn view_of_buffer(&self, key: usize) -> Option<&BufferView> {
        self.views.get(&key)
    }

    fn glyph_widths(&self) -> &HashMap<char, u8> {
        &self.glyphs
    }

    fn gutter_width(&self, _total_lines: usize) -> usize {
        self.gutter
    }

    fn panes(&self) -> &[Pane] {
        &self.panes
    }
}

impl ViewportHost for StubHost {
    fn option(&self, name: &str) -> Option<&str> {
        self.options.get(name).map(String::as_str)
    }

    fn panes(&self) -> &[Pane] {
        &self.panes
    }

    fn panes_mut(&mut self) -> &mut [Pane] {
        &mut self.panes
    }

    fn focused_window(&self) -> Option<WindowId> {
        self.focused
    }

    fn window(&self, id: WindowId) -> Option<&Pane> {
        self.panes.iter().find(|p| p.id == id)
    }

    fn pane_metrics(&self, index: usize) -> Option<PaneMetrics> {
        self.pane_metrics.get(index).copied().flatten()
    }
}

impl FrameHost for StubHost {
    fn theme_color(&self, key: &str) -> (u8, u8, u8) {
        self.theme.get(key).copied().unwrap_or((0, 0, 0))
    }

    fn panes(&self) -> &[Pane] {
        &self.panes
    }

    fn focused_window(&self) -> Option<WindowId> {
        self.focused
    }
}

#[test]
fn options_are_read_through_the_host() {
    let mut host = StubHost::with_text("");
    assert_eq!(int_option(&host, "tab-width", 4), 4, "missing option falls back");
    assert!(!bool_option(&host, "number"), "missing flag is false");

    host.options.insert("tab-width".into(), "8".into());
    host.options.insert("number".into(), "true".into());
    host.options.insert("wrap".into(), "1".into());
    host.options.insert("list".into(), "yes".into());

    assert_eq!(int_option(&host, "tab-width", 4), 8);
    assert!(bool_option(&host, "number"));
    assert!(bool_option(&host, "wrap"));
    assert!(!bool_option(&host, "list"), "only true/1 are truthy");
}

#[test]
fn scroll_state_is_derived_from_host_buffers_and_views() {
    let mut host = StubHost::with_text("aaa\nbb\nccc");
    host.options.insert("tab-width".into(), "4".into());

    let state = compute_scroll_state(&host, 0, 2);
    assert_eq!(state.cursor_line, 0);
    assert_eq!(state.scroll_top, 0);

    host.set_cursor(0, 8);
    let state = compute_scroll_state(&host, 0, 2);
    assert_eq!(state.cursor_line, 2, "offset 8 is the third line");
    assert_eq!(state.scroll_top, 2, "cursor leaves the window, viewport follows");
    assert_eq!(state.cursor_vis_row, 0);
}

#[test]
fn a_missing_buffer_falls_back_without_touching_the_editor() {
    let host = StubHost::with_text("");
    let state = compute_scroll_state(&host, 99, 10);
    assert_eq!(state.scroll_top, 0);
    assert_eq!(state.cursor_line, 0);
    assert_eq!(horizontal_extent(&host, 99), 0);
    assert_eq!(pane_scroll_top(&host, &Pane::new(WindowId(1)), 99, 10), 0);
}

#[test]
fn gutter_and_horizontal_extent_use_only_the_host() {
    let mut host = StubHost::with_text("aaa\nbb\nccc");
    host.gutter = 4;

    assert_eq!(gutter_cols(&host, 0, 3), 4, "gutter width comes from the host");

    host.panes.push(Pane {
        buffer_id: Some(0),
        height: 3,
        scroll_offset: 0,
        ..Pane::new(WindowId(1))
    });
    // Two visible rows (height minus the tab line): "aaa" is the widest.
    assert_eq!(horizontal_extent(&host, 0), 3);
}

#[test]
fn prepare_frame_seeds_surface_from_frame_host() {
    let mut host = StubHost::with_text("");
    host.theme.insert("fg".into(), (10, 20, 30));
    host.theme.insert("bg".into(), (40, 50, 60));

    let mut surface = Surface::new(2, 2);
    prepare_frame(&host, &mut surface);

    let cell = surface.cell(0, 0).unwrap();
    assert_eq!(cell.style.fg, (10, 20, 30));
    assert_eq!(cell.style.bg, (40, 50, 60));
}

#[test]
fn sync_viewport_scroll_runs_without_editor_when_host_has_no_panes() {
    let mut host = StubHost::with_text("");
    // No panes: nothing to sync, must not panic.
    sync_viewport_scroll(&mut host);
}

#[test]
fn frame_panes_skips_unfocused_tab_panes() {
    use crate::kernel::render::view_tree::LayoutConstraint;
    let mut host = StubHost::with_text("");
    let mut p1 = Pane::new(WindowId(1));
    p1.buffer_id = Some(0);
    p1.x = 0;
    p1.y = 0;
    p1.width = 10;
    p1.height = 5;
    let mut p2 = Pane::new(WindowId(2));
    p2.buffer_id = Some(1);
    p2.x = 0;
    p2.y = 0;
    p2.width = 10;
    p2.height = 5;
    p2.layout = LayoutConstraint::Tab;
    host.panes.push(p1);
    host.panes.push(p2);
    host.focused = Some(WindowId(1));

    let panes = frame_panes(&host);
    assert_eq!(panes.len(), 1, "unfocused tab pane is skipped");
    assert_eq!(panes[0].0.id, WindowId(1));
}

#[test]
fn render_overlays_paints_without_editor() {
    let host = StubHost::with_text("");
    let overlays = vec![Overlay {
        id: 1,
        x: 0,
        y: 0,
        width: 2,
        height: 1,
        buffer_id: None,
        z_order: 0,
    }];
    let mut surface = Surface::new(4, 2);
    render_overlays(&host, &overlays, (1, 2, 3), (4, 5, 6), &mut surface);
    // overlay background seeded
    assert_eq!(surface.cell(0, 0).unwrap().style.bg, (1, 2, 3));
}

#[test]
fn build_visible_rows_extracts_lines_without_editor() {
    let buf = Buffer::from_string(BufferId(0), "stub", "a\nb\nc");
    let rows = build_visible_rows(&buf, &[], 0, 2, buf.line_count());
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].text, "a\n");
    assert!(!rows[0].fold);
    let _ = VisibleRow { start: 0, end: 0, text: String::new(), fold: false };
}
