//! SidebarView — renders the project file tree from ProjectManager.

use crate::kernel::render::surface::{Style, Surface};
use crate::kernel::render::view::{View, ViewKind, RenderCtx};

pub struct SidebarView;

impl View for SidebarView {
    fn kind(&self) -> ViewKind { ViewKind::Sidebar }

    fn render(&self, surface: &mut Surface, ctx: &RenderCtx) {
        let editor = ctx.editor;
        let area = ctx.area;

        let bg = editor.theme_color("status-bg");
        let fg = editor.theme_color("status-fg");
        let style = Style { fg, bg, ..Default::default() };

        for row in 0..area.height {
            for col in 0..area.width {
                surface.set_cell(area.x + col, area.y + row, ' ', Some(style));
            }
        }

        surface.set_text(area.x, area.y, "Files", Some(style));

        let files = &editor.project_manager.project.files;
        for (i, path) in files.iter().enumerate() {
            let row = i as u16 + 1;
            if row >= area.height { break; }
            let truncated: String = path.chars().take(area.width as usize).collect();
            surface.set_text(area.x, area.y + row, &truncated, Some(style));
        }
    }
}
