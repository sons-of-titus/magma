//! Project file-tree sidebar widget for the GUI — JetBrains-style.
//!
//! Builds a `FileNode` tree from the flat `project_manager.project.files` list,
//! renders it with collapse/expand arrows, file-type icons, selection highlight,
//! and double-click-to-open.  Resize is handled by egui's `SidePanel`.

use std::collections::HashSet;
use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Rounding, Sense, Vec2};
use crate::kernel::state::Editor;

// ── Catppuccin Mocha colours ──────────────────────────────────────────────────
const HEADER_FG: Color32 = Color32::from_rgb(108, 112, 134); // overlay0
const DIR_FG:    Color32 = Color32::from_rgb(180, 190, 254); // lavender
const FILE_FG:   Color32 = Color32::from_rgb(186, 194, 222); // subtext1
const ACTIVE_FG: Color32 = Color32::from_rgb(205, 214, 244); // text
const SEL_BG:    Color32 = Color32::from_rgb(49,  50,  68);  // surface0

const ROW_H:     f32 = 22.0;
const INDENT:    f32 = 14.0;
const FONT_SIZE: f32 = 13.0;
const ARROW_W:   f32 = 14.0;

fn file_icon(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "rs"         => "🦀",
        "janet"      => "🟢",
        "py"         => "🐍",
        "toml"       => "⚙",
        "md"         => "📝",
        "json"       => "📋",
        "yaml" | "yml" => "⚡",
        "sh"         => "$_",
        "lock"       => "🔒",
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

// ── Sidebar state (persisted across frames in GuiApp) ─────────────────────────

/// Interaction state for the project tree sidebar.
pub struct SidebarState {
    pub expanded_dirs: HashSet<String>,
    pub selected:      Option<String>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            expanded_dirs: HashSet::new(),
            selected:      None,
        }
    }
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

    fn render_nodes(
        ui:           &mut egui::Ui,
        nodes:        &[FileNode],
        depth:        usize,
        state:        &mut SidebarState,
        project_root: &str,
    ) -> Option<String> {
        let font_id = FontId::new(FONT_SIZE, FontFamily::Proportional);
        let mut to_open: Option<String> = None;

        for node in nodes {
            let indent    = depth as f32 * INDENT + 4.0;
            let avail_w   = ui.available_width();
            let is_sel    = state.selected.as_deref() == Some(node.path.as_str());
            let is_exp    = node.is_dir && state.expanded_dirs.contains(&node.path);

            let (row, resp) =
                ui.allocate_exact_size(Vec2::new(avail_w, ROW_H), Sense::click());

            // Background
            if is_sel {
                ui.painter().rect_filled(row, Rounding::ZERO, SEL_BG);
            } else if resp.hovered() {
                ui.painter().rect_filled(
                    row, Rounding::ZERO,
                    Color32::from_rgba_unmultiplied(49, 50, 68, 100),
                );
            }

            let cy = row.center().y;
            let mut x = row.left() + indent;

            // Collapse/expand arrow for directories
            if node.is_dir {
                let arrow = if is_exp { "▾" } else { "▸" };
                ui.painter().text(
                    Pos2::new(x, cy),
                    egui::Align2::LEFT_CENTER,
                    arrow,
                    font_id.clone(),
                    DIR_FG,
                );
            }
            x += ARROW_W;

            // Icon
            let icon = if node.is_dir {
                if is_exp { "📂" } else { "📁" }
            } else {
                file_icon(&node.name)
            };
            let icon_w: f32 = ui.fonts(|f| {
                f.layout_no_wrap(icon.to_string(), font_id.clone(), ACTIVE_FG).size().x
            });
            ui.painter().text(
                Pos2::new(x, cy), egui::Align2::LEFT_CENTER,
                icon, font_id.clone(),
                if node.is_dir { DIR_FG } else { FILE_FG },
            );
            x += icon_w + 4.0;

            // Name
            let fg = if is_sel { ACTIVE_FG } else if node.is_dir { DIR_FG } else { FILE_FG };
            ui.painter().text(
                Pos2::new(x, cy), egui::Align2::LEFT_CENTER,
                &node.name, font_id.clone(), fg,
            );

            // Click: select + toggle dir
            if resp.clicked() {
                state.selected = Some(node.path.clone());
                if node.is_dir {
                    if is_exp {
                        state.expanded_dirs.remove(&node.path);
                    } else {
                        state.expanded_dirs.insert(node.path.clone());
                    }
                }
            }

            // Double-click: open file
            if resp.double_clicked() && !node.is_dir && to_open.is_none() {
                let full = if project_root.is_empty() {
                    node.path.clone()
                } else {
                    format!("{}/{}", project_root.trim_end_matches('/'), node.path)
                };
                to_open = Some(full);
            }

            // Recurse
            if node.is_dir && is_exp {
                if let Some(p) =
                    Self::render_nodes(ui, &node.children, depth + 1, state, project_root)
                {
                    to_open = to_open.or(Some(p));
                }
            }
        }
        to_open
    }

    /// Render the project tree sidebar.  Returns the path of a file to open when
    /// the user double-clicks it, or `None` otherwise.
    pub fn show(ui: &mut egui::Ui, editor: &Editor, state: &mut SidebarState) -> Option<String> {
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
                    .color(HEADER_FG)
                    .size(11.0)
                    .strong(),
            );
        });
        ui.add_space(2.0);
        ui.separator();

        let files = &editor.project_manager.project.files;
        if files.is_empty() {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("No project files indexed.")
                    .color(HEADER_FG)
                    .size(FONT_SIZE),
            );
            return None;
        }

        let tree = Self::build_tree(files);
        let mut to_open: Option<String> = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                to_open = Self::render_nodes(ui, &tree, 0, state, &project_root);
            });

        to_open
    }
}
