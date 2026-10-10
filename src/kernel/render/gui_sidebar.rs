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

const ROW_H:     f32 = 24.0;
const INDENT:    f32 = 14.0;
const FONT_SIZE: f32 = 13.0;
const ARROW_W:   f32 = 14.0;
const FOLDER_W:  f32 = 13.0;

/// Collapse/expand arrows — must be covered by egui's default fonts
/// (enforced by `every_sidebar_glyph_is_covered_by_egui_default_fonts`).
pub(crate) const ARROW_OPEN:   &str = "⏷";
pub(crate) const ARROW_CLOSED: &str = "⏵";

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

/// Painter-drawn folder glyph — crisp at any DPI and independent of the
/// bundled fonts' limited symbol coverage.
fn draw_folder(p: &egui::Painter, x: f32, cy: f32, open: bool, color: egui::Color32) {
    let color = if open { color.gamma_multiply(0.8) } else { color };
    let (w, h) = (11.0, 8.0);
    let tab = egui::Rect::from_min_size(
        Pos2::new(x, cy - h / 2.0 - 2.0),
        Vec2::new(w * 0.45, 3.0),
    );
    p.rect_filled(tab, CornerRadius { nw: 2, ne: 0, sw: 0, se: 0 }, color);
    p.rect_filled(
        egui::Rect::from_min_size(Pos2::new(x, cy - h / 2.0), Vec2::new(w, h)),
        CornerRadius::same(2),
        color,
    );
}

/// Small square utility button in the sidebar header.
fn header_button(ui: &mut egui::Ui, theme: &GuiTheme, glyph: &str, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4, theme.surface0.gamma_multiply(0.5));
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        FontId::new(13.0, FontFamily::Proportional),
        theme.subtext0,
    );
    resp.on_hover_text(tip).clicked()
}

// ── Tree data ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct FileNode {
    pub(crate) name:     String,
    pub(crate) path:     String,
    pub(crate) is_dir:   bool,
    pub(crate) children: Vec<FileNode>,
}

// ── Widget ────────────────────────────────────────────────────────────────────

pub struct ProjectTree;

impl ProjectTree {
    // ── Tree construction ─────────────────────────────────────────────────────

    /// Insert one indexed path (project-relative, `/`-separated) into the
    /// tree.  Every node's `path` is the joined relative prefix, so node
    /// identity matches `expanded_dirs` keys and the breadcrumb reveal.
    fn insert(nodes: &mut Vec<FileNode>, parts: &[&str], prefix: &str) {
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
                Self::insert(&mut existing.children, &parts[1..], &node_path);
            }
        } else if is_leaf {
            nodes.push(FileNode {
                name: name.to_string(),
                path: node_path,
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
            Self::insert(&mut dir.children, &parts[1..], &node_path);
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

    pub(crate) fn build_tree(files: &[String]) -> Vec<FileNode> {
        let mut roots: Vec<FileNode> = Vec::new();
        for path in files {
            let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            if parts.is_empty() { continue; }
            Self::insert(&mut roots, &parts, "");
        }
        Self::sort_nodes(&mut roots);
        roots
    }

    fn collect_dirs(nodes: &[FileNode], out: &mut std::collections::HashSet<String>) {
        for n in nodes {
            if n.is_dir {
                out.insert(n.path.clone());
                Self::collect_dirs(&n.children, out);
            }
        }
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

            // Background — inset, rounded, JetBrains-tree style.
            let row_fill = if is_sel {
                Some(theme.surface0)
            } else if resp.hovered() {
                Some(theme.surface0.gamma_multiply(0.5))
            } else {
                None
            };
            if let Some(fill) = row_fill {
                ui.painter().rect_filled(
                    row.shrink2(egui::vec2(3.0, 0.0)),
                    CornerRadius::same(4),
                    fill,
                );
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
                    theme.overlay0,
                );
            }
            x += ARROW_W;

            // Icon — painter-drawn folder for directories, glyph for files.
            if node.is_dir {
                draw_folder(ui.painter(), x + 1.0, cy, is_exp, theme.lavender);
                x += FOLDER_W + 4.0;
            } else {
                let icon = file_icon(&node.name);
                let icon_w: f32 = ui.fonts_mut(|f| {
                    f.layout_no_wrap(icon.to_string(), font_id.clone(), theme.text).size().x
                });
                ui.painter().text(
                    Pos2::new(x, cy), egui::Align2::LEFT_CENTER,
                    icon, font_id.clone(), theme.subtext1,
                );
                x += icon_w + 4.0;
            }

            // Name
            let fg = if is_sel { theme.text } else if node.is_dir { theme.lavender } else { theme.subtext1 };
            ui.painter().text(
                Pos2::new(x, cy), egui::Align2::LEFT_CENTER,
                &node.name, font_id.clone(), fg,
            );

            // Node paths are project-relative; actions need absolute paths.
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
                        full_path.clone()
                    } else {
                        full_path.rsplit_once('/').map(|(d, _)| d.to_string())
                            .unwrap_or_else(|| project_root.to_string())
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

        let files = &editor.project_manager.project.files;
        let epoch = editor.project_manager.project.files_epoch;
        if state.tree_cache.as_ref().map(|(e, _)| *e) != Some(epoch) {
            let tree = Self::build_tree(files);
            state.tree_cache = Some((epoch, std::rc::Rc::new(tree)));
        }
        let tree = state.tree_cache.clone().map(|(_, t)| t)
            .unwrap_or_default();

        // Header row — project name plus collapse/expand utilities.
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(project_name)
                    .color(theme.text)
                    .size(12.0)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if header_button(ui, theme, "+", "Expand all") {
                    state.expanded_dirs.clear();
                    Self::collect_dirs(&tree, &mut state.expanded_dirs);
                }
                if header_button(ui, theme, "-", "Collapse all") {
                    state.expanded_dirs.clear();
                }
            });
        });
        ui.add_space(2.0);
        ui.separator();

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

        // A different project root invalidates expansion/selection state;
        // the fresh tree starts expanded one level so the opened project's
        // contents are visible without clicking through parents.
        if state.root.as_deref() != Some(project_root.as_str()) {
            state.reset_for(project_root.clone());
            for node in tree.iter().filter(|n| n.is_dir) {
                state.expanded_dirs.insert(node.path.clone());
            }
        }

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
