//! Find-in-files tool window — project-wide content search.
//!
//! Enter runs ripgrep (`--vimgrep`) against the project root on a background
//! thread; results land in the shared quickfix list, so the panel, `:cn/:cp`,
//! and compiler output all navigate the same way.

use eframe::egui::{self, FontFamily, FontId, RichText};
use crate::kernel::state::Editor;
use super::gui_theme::GuiTheme;
use super::tool_window::{ToolAction, ToolWindow};

fn mono(size: f32) -> FontId { FontId::new(size, FontFamily::Monospace) }
fn prop(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

const MAX_RESULTS: usize = 500;

/// Parse one `--vimgrep` line: `path:line:col:text`.
fn parse_vimgrep(line: &str) -> Option<crate::kernel::task::QuickfixEntry> {
    let mut parts = line.splitn(4, ':');
    let filename = parts.next()?.to_string();
    let line_no: usize = parts.next()?.parse().ok()?;
    let col: usize = parts.next()?.parse().ok()?;
    let message = parts.next().unwrap_or("").to_string();
    Some(crate::kernel::task::QuickfixEntry { filename, line: line_no, col, message })
}

/// The shared results renderer for every quickfix consumer (find-in-files,
/// the output panel, `:cn/:cp`).  Rows are clickable.
pub(crate) fn quickfix_rows(ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) -> Vec<ToolAction> {
    let mut actions = Vec::new();
    if editor.quickfix_list.is_empty() {
        ui.label(RichText::new("No results.").font(prop(12.0)).color(theme.overlay0));
        return actions;
    }
    for (i, entry) in editor.quickfix_list.iter().enumerate() {
        let is_current = i == editor.quickfix_index;
        let short = entry.filename.rsplit('/').next().unwrap_or(&entry.filename);
        let label = RichText::new(format!(
            "{}:{}  {}",
            short, entry.line, entry.message.trim_end(),
        ))
        .font(mono(12.0))
        .color(if is_current { theme.text } else { theme.subtext1 });
        let resp = ui.add(
            egui::Button::new(label)
                .fill(if is_current { theme.surface0 } else { egui::Color32::TRANSPARENT })
                .frame(is_current),
        );
        if resp.clicked() {
            actions.push(ToolAction::Goto {
                path: entry.filename.clone(),
                line: entry.line,
            });
        }
    }
    actions
}

pub struct FindInFilesToolWindow {
    query:     String,
    /// A background search is in flight for `last_pattern`.
    searching: bool,
    last_pattern: String,
}

impl FindInFilesToolWindow {
    pub fn new() -> Self {
        Self { query: String::new(), searching: false, last_pattern: String::new() }
    }

    fn run_search(&mut self, editor: &Editor) {
        if self.query.is_empty() { return; }
        let Some(bg) = editor.background.clone() else { return };
        let root = editor.project_manager.project.root.clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let pattern = self.query.clone();
        self.searching = true;
        self.last_pattern = pattern.clone();
        let sender = bg.sender.clone();
        bg.spawn_blocking(move || {
            let output = std::process::Command::new("rg")
                .args(["--vimgrep", "--no-heading", &pattern, "."])
                .current_dir(&root)
                .output();
            let mut entries: Vec<crate::kernel::task::QuickfixEntry> = match output {
                Ok(o) => String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter_map(parse_vimgrep)
                    .collect(),
                Err(_) => Vec::new(),
            };
            entries.truncate(MAX_RESULTS);
            let _ = sender.send(crate::kernel::runtime::BackgroundEvent::GrepResults { entries });
        });
    }
}

impl ToolWindow for FindInFilesToolWindow {
    fn title(&self) -> &str { "Find in Files" }
    fn icon(&self)  -> char { super::gui_theme::icons::SEARCH.chars().next().unwrap_or('⌕') }

    fn ui(&mut self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .hint_text("Search project (ripgrep)…  Enter to run")
                .font(mono(13.0))
                .desired_width(f32::INFINITY),
        );
        let enter = resp.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(
                RichText::new("Search").font(prop(12.0)))
            ).clicked() {
                self.run_search(editor);
            }
            if self.searching {
                ui.label(RichText::new(format!("searching \"{}\"…", self.last_pattern))
                    .font(prop(11.0)).color(theme.overlay0));
            }
        });
        if enter {
            self.run_search(editor);
        }
        ui.separator();

        if editor.quickfix_list.is_empty() {
            ui.label(
                RichText::new("No results yet.  Type a pattern and press Enter.")
                    .font(prop(12.0))
                    .color(theme.overlay0),
            );
            return Vec::new();
        }
        ui.label(
            RichText::new(format!("{} matches — click to jump, :cn/:cp to step", editor.quickfix_list.len()))
                .font(prop(11.0))
                .color(theme.overlay0),
        );
        quickfix_rows(ui, editor, theme)
    }
}
