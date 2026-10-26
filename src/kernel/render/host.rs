//! `RenderHost` — the read-only editor surface the frame layer needs.
//!
//! The frame helpers used to take the concrete `Editor`, which pinned text
//! layout, scrolling and gutter math to the whole editor type.  They only ever
//! read six things: an option, a buffer, a buffer's view, the font's glyph
//! widths, the gutter width, and the pane list.  `RenderHost` names exactly
//! that, so the geometry code can be exercised with a stub and moved out of
//! the kernel without dragging `Editor` along.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::kernel::render::view_tree::Pane;
use crate::kernel::state::Editor;
use crate::kernel::text_engine::{Buffer, BufferView};

pub trait RenderHost {
    /// Raw option value; interpretation is the caller's job.
    fn option(&self, name: &str) -> Option<&str>;
    fn buffer(&self, key: usize) -> Option<&Arc<Mutex<Buffer>>>;
    fn view_of_buffer(&self, key: usize) -> Option<&BufferView>;
    fn glyph_widths(&self) -> &HashMap<char, u8>;
    fn gutter_width(&self, total_lines: usize) -> usize;
    fn panes(&self) -> &[Pane];
}

impl RenderHost for Editor {
    fn option(&self, name: &str) -> Option<&str> {
        self.options.get(name).map(String::as_str)
    }

    fn buffer(&self, key: usize) -> Option<&Arc<Mutex<Buffer>>> {
        self.buffers.get(key)
    }

    fn view_of_buffer(&self, key: usize) -> Option<&BufferView> {
        Editor::view_of_buffer(self, key)
    }

    fn glyph_widths(&self) -> &HashMap<char, u8> {
        &self.font_config.glyph_widths
    }

    fn gutter_width(&self, total_lines: usize) -> usize {
        self.gutter.total_width(total_lines)
    }

    fn panes(&self) -> &[Pane] {
        self.view_tree.panes()
    }
}
