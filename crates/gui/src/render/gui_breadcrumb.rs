//! Breadcrumb navigation bar — Phase 6.
//!
//! A horizontal strip above the editor showing the focused buffer's path as
//! clickable "›"-separated segments, plus a symbol tail naming the innermost
//! indexed symbol at the cursor.  Clicking a path segment asks the sidebar to
//! reveal that directory.

use eframe::egui::{self, FontFamily, FontId, Sense};
use crate::kernel::state::Editor;
use super::gui_theme::{GuiTheme, icons};

const BAR_H:     f32 = 22.0;
const FONT_SIZE: f32 = 12.0;

/// Action the caller applies after the bar renders.
pub enum BreadcrumbAction {
    /// Expand the sidebar so `rel_dir` (project-relative, "/"-joined) is visible.
    RevealInSidebar { rel_dir: String },
}

/// Split a path into display segments: `src/kernel/mod.rs` → `["src", "kernel", "mod.rs"]`.
pub fn segments(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Strip `root` off the front of `path`, returning a project-relative path.
pub fn relative_to_project(root: Option<&std::path::Path>, path: &str) -> String {
    if let Some(root) = root {
        let root_str = root.to_string_lossy().trim_end_matches('/').to_string();
        if let Some(stripped) = path.strip_prefix(&root_str) {
            return stripped.trim_start_matches('/').to_string();
        }
    }
    path.to_string()
}

/// Innermost indexed symbol whose definition line is at or before `cursor_line`.
/// Symbol entries carry no end line, so "innermost" is the closest preceding
/// definition — accurate for non-nested styles, approximate otherwise.
pub fn symbol_at(editor: &Editor, path: &str, cursor_line: usize) -> Option<String> {
    editor.semantic.symbol_index.all_symbols().iter()
        .filter(|s| s.file == path)
        .filter(|s| s.line <= cursor_line)
        .max_by_key(|s| s.line)
        .map(|s| s.name.clone())
}

pub struct BreadcrumbBar;

impl BreadcrumbBar {
    /// Render the breadcrumb bar.  Returns a sidebar-reveal action when a
    /// path segment is clicked.
    pub fn show(ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) -> Option<BreadcrumbAction> {
        let wid    = editor.view_tree.focused_window()?;
        let buf_id = editor.view_tree.buffer(wid)?;
        let arc    = editor.buffers.get(buf_id)?;

        let (path, cursor_offset) = {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            let cursor = editor.view_of_buffer(buf_id).map(|v| v.cursor_offset()).unwrap_or(0);
            // Prefer the full path so the bar shows the JetBrains-style
            // directory chain; scratch buffers fall back to their name.
            (buf.path.clone().unwrap_or_else(|| buf.name.clone()), cursor)
        };

        let cursor_line = {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            buf.position_of(cursor_offset).line
        };

        let root = editor.project_manager.project.root.as_deref();
        let rel  = relative_to_project(root, &path);
        let segs = segments(&rel);

        ui.horizontal(|ui| {
            ui.set_min_height(BAR_H);
            // The breadcrumb is part of the editor surface — same fill — and
            // tucked under the tab strip, per the JetBrains layout.
            ui.painter().rect_filled(ui.max_rect(), 0, theme.base);
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(4.0);

            let font = FontId::new(FONT_SIZE, FontFamily::Proportional);
            let mut action = None;

            for (i, seg) in segs.iter().enumerate() {
                if i > 0 {
                    ui.label(
                        egui::RichText::new(icons::SEPARATOR)
                            .font(font.clone())
                            .color(theme.overlay0),
                    );
                }
                let is_file = i == segs.len() - 1;
                let fg = if is_file { theme.subtext1 } else { theme.overlay0 };
                let resp = ui.add(
                    egui::Label::new(
                        egui::RichText::new(seg).font(font.clone()).color(fg)
                    ).sense(Sense::click()),
                ).on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.hovered() {
                    let r = resp.rect;
                    ui.painter().rect_filled(
                        r.expand2(egui::vec2(3.0, 1.0)),
                        egui::CornerRadius::same(4),
                        theme.surface0.gamma_multiply(0.5),
                    );
                    // Re-paint the label above the hover fill.
                    ui.painter().text(
                        r.center(),
                        egui::Align2::CENTER_CENTER,
                        seg,
                        font.clone(),
                        theme.text,
                    );
                }
                if resp.clicked() && action.is_none() {
                    action = Some(BreadcrumbAction::RevealInSidebar {
                        rel_dir: segs[..=i].join("/"),
                    });
                }
            }

            if let Some(sym) = symbol_at(editor, &path, cursor_line) {
                ui.label(
                    egui::RichText::new(icons::SEPARATOR)
                        .font(font.clone())
                        .color(theme.overlay0),
                );
                ui.label(
                    egui::RichText::new(format!("ƒ {}", sym))
                        .font(FontId::new(FONT_SIZE - 1.0, FontFamily::Proportional))
                        .color(theme.lavender)
                        .background_color(theme.surface0),
                );
            }

            action
        })
        .inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_split_path() {
        assert_eq!(segments("src/kernel/mod.rs"), vec!["src", "kernel", "mod.rs"]);
        assert_eq!(segments("/abs/path"), vec!["abs", "path"]);
        assert!(segments("").is_empty());
    }

    #[test]
    fn relative_strips_root() {
        let root = std::path::Path::new("/Users/x/magma");
        assert_eq!(relative_to_project(Some(root), "/Users/x/magma/src/main.rs"), "src/main.rs");
        assert_eq!(relative_to_project(Some(root), "/elsewhere/a.rs"), "/elsewhere/a.rs");
        assert_eq!(relative_to_project(None, "plain/a.rs"), "plain/a.rs");
    }
}
