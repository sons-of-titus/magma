//! TerminalView — renders PTY buffer content.

use crate::kernel::render::surface::{Style, Surface};
use crate::kernel::render::view::{View, ViewKind, RenderCtx};

pub struct TerminalView {
    pub buf_id: usize,
}

impl View for TerminalView {
    fn kind(&self) -> ViewKind { ViewKind::Terminal }

    fn render(&self, surface: &mut Surface, ctx: &RenderCtx) {
        let editor = ctx.editor;
        let area = ctx.area;

        let arc = match editor.buffers.get(self.buf_id) {
            Some(a) => a,
            None => return,
        };
        let buf = arc.lock().unwrap();

        let tf = editor.theme_color("terminal-fg");
        let t_style = Style { fg: tf, bg: (0, 0, 0), ..Default::default() };
        let total_lines = buf.line_count();
        let text = buf.slice(0, buf.len());
        let visible_lines = area.height as usize;
        let scroll_top = total_lines.saturating_sub(visible_lines);
        let max_visible = (total_lines - scroll_top).min(visible_lines);
        for i in 0..max_visible {
            let abs_line = scroll_top + i;
            let line_start = buf.line_start_offset(abs_line).unwrap_or(text.len());
            let line_end   = buf.line_start_offset(abs_line + 1).unwrap_or(text.len());
            let line_text  = &text[line_start..line_end];
            for (col, ch) in line_text.chars().enumerate().take(area.width as usize) {
                surface.set_cell(area.x + col as u16, area.y + i as u16, ch, Some(t_style));
            }
        }
    }
}
