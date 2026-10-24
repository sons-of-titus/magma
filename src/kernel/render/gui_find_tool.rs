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

use crate::kernel::task::QUICKFIX_MAX_RESULTS as MAX_RESULTS;

/// Parse one `--vimgrep` line: `path:line:col:text`.
fn parse_vimgrep(line: &str) -> Option<crate::kernel::task::QuickfixEntry> {
    let mut parts = line.splitn(4, ':');
    let filename = parts.next()?.to_string();
    let line_no: usize = parts.next()?.parse().ok()?;
    let col: usize = parts.next()?.parse().ok()?;
    let message = parts.next().unwrap_or("").to_string();
    Some(crate::kernel::task::QuickfixEntry { filename, line: line_no, col, message, ..Default::default() })
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
    /// Kill handle for the running `rg` — a newer search cancels the
    /// older one instead of racing it (rework.md §6).
    running: std::sync::Arc<std::sync::Mutex<Option<std::process::Child>>>,
}

impl FindInFilesToolWindow {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            searching: false,
            last_pattern: String::new(),
            running: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    fn run_search(&mut self, editor: &mut Editor) {
        if self.query.is_empty() { return; }
        let Some(bg) = editor.background.clone() else { return };
        let root = editor.project_manager.project.root.clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let pattern = self.query.clone();
        self.searching = true;
        self.last_pattern = pattern.clone();

        // Cancel any in-flight search: kill its rg and void its results.
        if let Ok(mut guard) = self.running.lock()
            && let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        editor.search_generation += 1;
        let generation = editor.search_generation;
        editor.quickfix_list.clear();

        let sender = bg.sender.clone();
        let running = self.running.clone();
        bg.spawn_blocking(move || {
            let Ok(child) = std::process::Command::new("rg")
                .args(["--vimgrep", "--no-heading", &pattern, "."])
                .current_dir(&root)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
            else { return };
            *running.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);

            // Stream results in batches so the quickfix list fills while
            // rg walks, instead of after the whole project.
            let mut stdout = running.lock().unwrap_or_else(|e| e.into_inner())
                .as_mut()
                .and_then(|c| c.stdout.take());
            let mut batch: Vec<crate::kernel::task::QuickfixEntry> = Vec::new();
            let mut total = 0usize;
            if let Some(out) = stdout.as_mut() {
                use std::io::BufRead;
                for line in std::io::BufReader::new(&mut *out).lines().map_while(Result::ok) {
                    if let Some(entry) = parse_vimgrep(&line) {
                        batch.push(entry);
                        total += 1;
                    }
                    if total >= MAX_RESULTS { break; }
                    if batch.len() >= 100 {
                        let _ = sender.send(crate::kernel::runtime::BackgroundEvent::GrepResultsBatch {
                            generation, entries: std::mem::take(&mut batch),
                        });
                    }
                }
            }
            if !batch.is_empty() {
                let _ = sender.send(crate::kernel::runtime::BackgroundEvent::GrepResultsBatch {
                    generation, entries: batch,
                });
            }
            let _ = sender.send(crate::kernel::runtime::BackgroundEvent::GrepDone { generation });
            if let Ok(mut guard) = running.lock() {
                if let Some(child) = guard.as_mut() {
                    let _ = child.wait();
                }
                *guard = None;
            }
        });
    }
}

impl ToolWindow for FindInFilesToolWindow {
    fn title(&self) -> &str { "Find in Files" }
    fn icon(&self)  -> char { super::gui_theme::icons::SEARCH.chars().next().unwrap_or('⌕') }

    fn ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
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
