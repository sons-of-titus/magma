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

use crate::kernel::render::frame::{content_width, cursor_metrics, int_option};
use crate::kernel::render::view_tree::Pane;
use crate::kernel::state::Editor;
use crate::kernel::state::id::WindowId;
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

/// Everything one viewport-sync pass needs, read without cloning buffer
/// handles and written through `panes_mut`.
pub trait ViewportHost {
    /// Raw option value; interpretation is the caller's job.
    fn option(&self, name: &str) -> Option<&str>;
    fn panes(&self) -> &[Pane];
    fn panes_mut(&mut self) -> &mut [Pane];
    fn focused_window(&self) -> Option<WindowId>;
    fn window(&self, id: WindowId) -> Option<&Pane>;
    /// Cursor and line metrics for pane `index`, computed against its buffer.
    fn pane_metrics(&self, index: usize) -> Option<PaneMetrics>;
}

/// Cursor and geometry state for one pane, copied out so the mutable pane
/// borrow can begin without holding a buffer lock or an `Editor` borrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneMetrics {
    pub buffer_id: usize,
    pub cursor_offset: usize,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub total_lines: usize,
    pub height: u16,
    pub width: u16,
    pub content_width: usize,
}

impl ViewportHost for Editor {
    fn option(&self, name: &str) -> Option<&str> {
        RenderHost::option(self, name)
    }

    fn panes(&self) -> &[Pane] {
        self.view_tree.panes()
    }

    fn panes_mut(&mut self) -> &mut [Pane] {
        self.view_tree.panes_mut()
    }

    fn focused_window(&self) -> Option<WindowId> {
        self.view_tree.focused_window()
    }

    fn window(&self, id: WindowId) -> Option<&Pane> {
        self.view_tree.window(id)
    }

    fn pane_metrics(&self, index: usize) -> Option<PaneMetrics> {
        let pane = self.view_tree.panes().get(index)?;
        let buffer_id = pane.buffer_id?;
        let cursor_offset = self
            .view_of_buffer(buffer_id)
            .map(|v| v.cursor.offset)
            .unwrap_or(0);
        let arc = self.buffers.get(buffer_id)?;
        let (total_lines, cursor_line, cursor_col) = {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            let (line, col) = cursor_metrics(
                &buf,
                cursor_offset.min(buf.len()),
                int_option(self, "tab-width", 4),
                &self.font_config.glyph_widths,
            );
            (buf.line_count(), line, col)
        };
        let content_width = content_width(self, pane.width, buffer_id, total_lines);
        Some(PaneMetrics {
            buffer_id,
            cursor_offset,
            cursor_line,
            cursor_col,
            total_lines,
            height: pane.height,
            width: pane.width,
            content_width,
        })
    }
}

/// The surface-preparation half of the frame: theme colors and pane layout,
/// with no text, selection, or buffer access.
pub trait FrameHost {
    fn theme_color(&self, key: &str) -> (u8, u8, u8);
    fn panes(&self) -> &[Pane];
    fn focused_window(&self) -> Option<WindowId>;
}

impl FrameHost for Editor {
    fn theme_color(&self, key: &str) -> (u8, u8, u8) {
        Editor::theme_color(self, key)
    }

    fn panes(&self) -> &[Pane] {
        self.view_tree.panes()
    }

    fn focused_window(&self) -> Option<WindowId> {
        self.view_tree.focused_window()
    }
}
