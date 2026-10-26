//! Regression tests for `RenderHost` — the read-only capability surface the
//! frame layer uses instead of the concrete `Editor`.
//!
//! The stub below is deliberately not an `Editor`: it owns six pieces of data
//! and nothing else.  If the frame helpers ever reach for editor state beyond
//! the trait, these tests stop compiling — which is the point of the seam.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::kernel::render::frame::{
    bool_option, compute_scroll_state, gutter_cols, horizontal_extent, int_option, pane_scroll_top,
};
use crate::kernel::render::host::RenderHost;
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
