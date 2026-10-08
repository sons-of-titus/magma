//! Project file-tree sidebar widget for the GUI — JetBrains-style.
//!
//! Builds a `FileNode` tree from the flat `project_manager.project.files` list.
//! One activation convention: single-click opens files, single-click toggles
//! directories.  Rows carry a right-click context menu (open, copy path,
//! rename, delete); new/rename collect their name in an inline text row.
//! Returns `SidebarAction`s for the shell to apply.

use eframe::egui::{self, FontFamily, FontId, Pos2, CornerRadius, RichText, Sense, Vec2};
use crate::kernel::state::Editor;
use super::gui_sidebar_state::{PendingOp, SidebarAction, SidebarState};
use super::gui_theme::GuiTheme;

const ROW_H:     f32 = 22.0;
const INDENT:    f32 = 14.0;
const FONT_SIZE: f32 = 13.0;
const ARROW_W:   f32 = 14.0;

/// Collapse/expand arrows — must be covered by egui's default fonts
/// (enforced by `every_theme_icon_glyph_is_covered_by_egui_default_fonts`).
pub(crate) const ARROW_OPEN:   &str = "⏷";
pub(crate) const ARROW_CLOSED: &str = "⏵";
pub(crate) const DIR_OPEN:     &str = "○";
pub(crate) const DIR_CLOSED:   &str = "☐";

pub(crate) fn file_icon(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "rs"         => "⚙",
        "janet"      => "•",
        "py"         => "λ",
        "toml"       => "☰",
        "md"         => "§",
        "json"       => "∑",
        "yaml" | "yml" => "☰",
        "sh"         => "$_",
        "lock"       => "⊗",
        _            => "·",
    }
}

// ── Tree data ─────────────────────────────────────────────────────────────────

struct FileNode {
    name:     String,
    path:     String,
    is_dir:   bool,
    children: Vec<FileNode>,
}

// ── Widget ────────────────────────────────────────────────────────────────────

pub struct ProjectTree;

impl ProjectTree {
    // ── Tree construction ─────────────────────────────────────────────────────

    fn insert(nodes: &mut Vec<FileNode>, parts: &[&str], full_path: &str, prefix: &str) {
        if parts.is_empty() { return; }
        let name = parts[0];
        let is_leaf = parts.len() == 1;
        let node_path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", prefix, name)
        };

        if let Some(existing) = nodes.iter_mut().find(|n| n.name == name) {
            if !is_leaf {
                Self::insert(&mut existing.children, &parts[1..], full_path, &node_path);
            }
        } else if is_leaf {
            nodes.push(FileNode {
                name: name.to_string(),
                path: full_path.to_string(),
                is_dir: false,
                children: vec![],
            });
        } else {
            let mut dir = FileNode {
                name: name.to_string(),
                path: node_path.clone(),
                is_dir: true,
                children: vec![],
            };
            Self::insert(&mut dir.children, &parts[1..], full_path, &node_path);
            nodes.push(dir);
        }
    }

    fn sort_nodes(nodes: &mut Vec<FileNode>) {
        nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _             => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });
        for n in nodes.iter_mut().filter(|n| n.is_dir) {
            Self::sort_nodes(&mut n.children);
        }
    }

    fn build_tree(files: &[String]) -> Vec<FileNode> {
        let mut roots: Vec<FileNode> = Vec::new();
        for path in files {
            let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            if parts.is_empty() { continue; }
            Self::insert(&mut roots, &parts, path, "");
        }
        Self::sort_nodes(&mut roots);
        roots
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn render_nodes(
        ui:           &mut egui::Ui,
        nodes:        &[FileNode],
        depth:        usize,
        state:        &mut SidebarState,
        project_root: &str,
        theme:        &GuiTheme,
    ) -> Vec<SidebarAction> {
        let font_id = FontId::new(FONT_SIZE, FontFamily::Proportional);
        let mut actions = Vec::new();

        for node in nodes {
            let indent    = depth as f32 * INDENT + 4.0;
            let avail_w   = ui.available_width();
            let is_sel    = state.selected.as_deref() == Some(node.path.as_str());
            let is_exp    = node.is_dir && state.expanded_dirs.contains(&node.path);

            let (row, resp) =
                ui.allocate_exact_size(Vec2::new(avail_w, ROW_H), Sense::click());

            // Background
            if is_sel {
                ui.painter().rect_filled(row, CornerRadius::ZERO, theme.surface0);
            } else if resp.hovered() {
                ui.painter().rect_filled(row, CornerRadius::ZERO, theme.surface1);
            }

            let cy = row.center().y;
            let mut x = row.left() + indent;

            // Collapse/expand arrow for directories
            if node.is_dir {
                let arrow = if is_exp { ARROW_OPEN } else { ARROW_CLOSED };
                ui.painter().text(
                    Pos2::new(x, cy),
                    egui::Align2::LEFT_CENTER,
                    arrow,
                    font_id.clone(),
                    theme.lavender,
                );
            }
            x += ARROW_W;

            // Icon
            let icon = if node.is_dir {
                if is_exp { DIR_OPEN } else { DIR_CLOSED }
            } else {
                file_icon(&node.name)
            };
            let icon_w: f32 = ui.fonts_mut(|f| {
                f.layout_no_wrap(icon.to_string(), font_id.clone(), theme.text).size().x
            });
            ui.painter().text(
                Pos2::new(x, cy), egui::Align2::LEFT_CENTER,
                icon, font_id.clone(),
                if node.is_dir { theme.lavender } else { theme.subtext1 },
            );
            x += icon_w + 4.0;

            // Name
            let fg = if is_sel { theme.text } else if node.is_dir { theme.lavender } else { theme.subtext1 };
            ui.painter().text(
                Pos2::new(x, cy), egui::Align2::LEFT_CENTER,
                &node.name, font_id.clone(), fg,
            );

            let full_path = if project_root.is_empty() {
                node.path.clone()
            } else {
                format!("{}/{}", project_root.trim_end_matches('/'), node.path)
            };

            // One activation convention: single click opens files and
            // toggles directories.
            if resp.clicked() {
                state.selected = Some(node.path.clone());
                if node.is_dir {
                    if is_exp {
                        state.expanded_dirs.remove(&node.path);
                    } else {
                        state.expanded_dirs.insert(node.path.clone());
                    }
                } else {
                    actions.push(SidebarAction::Open(full_path.clone()));
                }
            }

            // Context menu — the discoverability surface for file operations.
            resp.context_menu(|ui| {
                state.selected = Some(node.path.clone());
                if !node.is_dir && ui.button("Open").clicked() {
                    actions.push(SidebarAction::Open(full_path.clone()));
                    ui.close();
                }
                if ui.button("Copy Path").clicked() {
                    actions.push(SidebarAction::CopyPath(full_path.clone()));
                    ui.close();
                }
                if ui.button("New File…").clicked() {
                    let dir = if node.is_dir {
                        node.path.clone()
                    } else {
                        node.path.rsplit_once('/').map(|(d, _)| d.to_string())
                            .unwrap_or_default()
                    };
                    state.pending = Some(PendingOp::NewFile { dir });
                    ui.close();
                }
                if ui.button("Rename…").clicked() {
                    state.pending = Some(PendingOp::Rename { full_path: full_path.clone() });
                    ui.close();
                }
                ui.separator();
                if ui.button("Delete").clicked() {
                    state.confirm_delete = Some(full_path.clone());
                    ui.close();
                }
                if state.confirm_delete.as_deref() == Some(full_path.as_str())
                    && ui.button(
                        RichText::new("Really delete?").color(theme.red)
                    ).clicked() {
                    actions.push(SidebarAction::Delete(full_path.clone()));
                    state.confirm_delete = None;
                    ui.close();
                }
            });

            // Recurse
            if node.is_dir && is_exp {
                actions.extend(Self::render_nodes(
                    ui, &node.children, depth + 1, state, project_root, theme,
                ));
            }
        }
        actions
    }

    /// Render the project tree sidebar.  Returns the user's requested
    /// actions for the shell to apply.
    pub fn show(
        ui:     &mut egui::Ui,
        editor: &Editor,
        state:  &mut SidebarState,
        theme:  &GuiTheme,
    ) -> Vec<SidebarAction> {
        let project_name = editor.project_manager.project.name
            .as_deref()
            .unwrap_or("Project");
        let project_root = editor.project_manager.project.root
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();

        // Header row
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(project_name.to_ascii_uppercase())
                    .color(theme.overlay0)
                    .size(11.0)
                    .strong(),
            );
        });
        ui.add_space(2.0);
        ui.separator();

        let files = &editor.project_manager.project.files;
        if files.is_empty() {
            ui.add_space(8.0);
            if editor.project_manager.project.root.is_none() {
                ui.label(
                    egui::RichText::new("No project open.")
                        .color(theme.overlay0)
                        .size(FONT_SIZE),
                );
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("Run :open <directory>, or pick one on the welcome screen")
                        .color(theme.overlay0)
                        .size(11.0),
                );
            } else {
                ui.label(
                    egui::RichText::new("Indexing project…")
                        .color(theme.overlay0)
                        .size(FONT_SIZE),
                );
            }
            return Vec::new();
        }

        let tree = Self::build_tree(files);
        let mut actions: Vec<SidebarAction> = Vec::new();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                actions = Self::render_nodes(ui, &tree, 0, state, &project_root, theme);
            });

        // Inline new/rename input row.
        if state.pending.is_some() {
            ui.separator();
            let hint = state.pending.as_ref().map(|o| o.hint()).unwrap_or_default();
            ui.label(
                egui::RichText::new(hint)
                    .color(theme.overlay0)
                    .size(11.0),
            );
            let mut name = state.pending_name.clone();
            let resp = ui.add(
                egui::TextEdit::singleline(&mut name)
                    .font(FontId::new(FONT_SIZE, FontFamily::Monospace))
                    .desired_width(f32::INFINITY),
            );
            resp.request_focus();
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
            let op = state.pending.take();
            if esc {
                state.pending_name.clear();
            } else if enter && !name.is_empty() {
                match op {
                    Some(PendingOp::NewFile { dir }) => {
                        actions.push(SidebarAction::CreateFile { dir, name });
                    }
                    Some(PendingOp::Rename { full_path }) => {
                        let to = match full_path.rsplit_once('/') {
                            Some((d, _)) => format!("{d}/{name}"),
                            None => name,
                        };
                        actions.push(SidebarAction::Rename { from: full_path, to });
                    }
                    None => {}
                }
                state.pending_name.clear();
            } else {
                state.pending = op;
                state.pending_name = name;
            }
        }

        actions
    }
}
