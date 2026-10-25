//! Commit tool window — the JetBrains-style Version Control panel: change
//! lists (staged/unstaged/untracked/conflicts) with click-to-stage, a branch
//! widget, a commit box, and an inline diff preview.
//!
//! Reads `editor.vc.last_status` / `preview` / `branches`; returns
//! `CommitAction`s the shell applies at its write boundary.

use eframe::egui::{self, FontFamily, FontId, RichText, Sense, Vec2};

use crate::kernel::state::Editor;
use crate::kernel::vc::VcStatus;

use super::gui_theme::GuiTheme;

const ROW_H: f32 = 22.0;
const FONT_SIZE: f32 = 12.5;

/// A Commit-window request, applied by the shell.
pub enum CommitAction {
    Stage(String),
    Unstage(String),
    Discard(String),
    ShowDiff(String),
    Commit { message: String, amend: bool },
    Refresh,
    BranchList,
    Checkout(String),
    NewBranch(String),
    OpenFile(String),
    CopyPath(String),
}

/// Commit-window UI state (lives on `GuiApp`, like `SidebarState`).
#[derive(Default)]
pub struct CommitState {
    pub message: String,
    pub amend: bool,
    pub selected: Option<String>,
    pub new_branch: String,
    pub show_diff: bool,
    /// Path the diff was last requested for — avoids re-requesting each frame.
    diff_for: Option<String>,
    /// Whether the branch list was already fetched for this repo.
    branches_for: Option<String>,
}

impl CommitState {
    /// Reset per-repo state when the repository root changes.
    pub fn sync_root(&mut self, root: Option<&str>) {
        if self.branches_for.as_deref() != root {
            self.branches_for = root.map(str::to_string);
            self.selected = None;
            self.diff_for = None;
        }
    }
}

pub struct CommitPanel;

impl CommitPanel {
    pub fn show(
        ui:     &mut egui::Ui,
        editor: &Editor,
        theme:  &GuiTheme,
        state:  &mut CommitState,
    ) -> Vec<CommitAction> {
        let mut actions = Vec::new();
        let root = editor.vc.repo_root.as_ref().map(|p| p.to_string_lossy().into_owned());
        state.sync_root(root.as_deref());

        let Some(status) = editor.vc.last_status.as_ref() else {
            ui.add_space(16.0);
            ui.label(RichText::new("No repository status yet.").color(theme.overlay0).size(FONT_SIZE));
            ui.label(RichText::new("Open a file in a Git repo, or press Refresh.")
                .color(theme.overlay0).size(11.0));
            if ui.button("Refresh").clicked() { actions.push(CommitAction::Refresh); }
            return actions;
        };

        // Fetch branches on first sight of this repo.
        if editor.vc.branches.is_empty() {
            actions.push(CommitAction::BranchList);
        }

        Self::header(ui, editor, status, theme, state, &mut actions);
        ui.separator();

        // Change lists.
        let mut diff_clicked: Option<String> = None;
        egui::ScrollArea::vertical()
            .id_salt("commit_changes")
            .auto_shrink([false, false])
            .max_height(220.0)
            .show(ui, |ui| {
                let conflicts: Vec<(String, char)> =
                    status.conflicts.iter().map(|p| (p.clone(), '!')).collect();
                Self::section(ui, theme, "Conflicts", &conflicts,
                    false, state, &mut actions, &mut diff_clicked, true);
                Self::section(ui, theme, "Staged", &status_codes(&status.staged),
                    true, state, &mut actions, &mut diff_clicked, false);
                Self::section(ui, theme, "Unstaged", &status_codes(&status.unstaged),
                    false, state, &mut actions, &mut diff_clicked, false);
                let untracked: Vec<(String, char)> =
                    status.untracked.iter().map(|p| (p.clone(), '?')).collect();
                Self::section(ui, theme, "Untracked", &untracked,
                    false, state, &mut actions, &mut diff_clicked, false);
            });

        if let Some(path) = diff_clicked {
            state.selected = Some(path.clone());
            if state.diff_for.as_deref() != Some(path.as_str()) {
                state.diff_for = Some(path.clone());
                actions.push(CommitAction::ShowDiff(path));
            }
        }

        ui.separator();
        Self::commit_box(ui, theme, state, &mut actions);
        Self::preview(ui, editor, theme, state);
        actions
    }

    fn header(
        ui:      &mut egui::Ui,
        editor:  &Editor,
        status:  &VcStatus,
        theme:   &GuiTheme,
        state:   &mut CommitState,
        actions: &mut Vec<CommitAction>,
    ) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&status.branch).strong().color(theme.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("⟳").on_hover_text("Refresh status").clicked() {
                    actions.push(CommitAction::Refresh);
                }
            });
        });
        ui.horizontal(|ui| {
            let mut chosen = status.branch.clone();
            egui::ComboBox::from_id_salt("commit_branch")
                .selected_text(status.branch.clone())
                .width(150.0)
                .show_ui(ui, |ui| {
                    for b in &editor.vc.branches {
                        ui.selectable_value(&mut chosen, b.clone(), b);
                    }
                    if editor.vc.branches.is_empty() {
                        ui.label(RichText::new("no branches").size(11.0).color(theme.overlay0));
                    }
                });
            if chosen != status.branch {
                actions.push(CommitAction::Checkout(chosen));
            }
            let resp = ui.add(egui::TextEdit::singleline(&mut state.new_branch)
                .hint_text("new branch")
                .desired_width(110.0));
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter && !state.new_branch.trim().is_empty() {
                actions.push(CommitAction::NewBranch(state.new_branch.trim().to_string()));
                state.new_branch.clear();
            }
        });
    }

    fn commit_box(
        ui:      &mut egui::Ui,
        theme:   &GuiTheme,
        state:   &mut CommitState,
        actions: &mut Vec<CommitAction>,
    ) {
        ui.label(RichText::new("Commit message").size(11.0).color(theme.overlay0));
        ui.add(egui::TextEdit::multiline(&mut state.message)
            .hint_text("Describe the change…")
            .desired_rows(3)
            .desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            ui.checkbox(&mut state.amend, "Amend");
            let enabled = !state.message.trim().is_empty() || state.amend;
            if ui.add_enabled(enabled, egui::Button::new(
                RichText::new(if state.amend { "Amend" } else { "Commit" }).strong()))
                .clicked() {
                actions.push(CommitAction::Commit {
                    message: state.message.clone(),
                    amend: state.amend,
                });
                state.message.clear();
                state.amend = false;
            }
        });
    }

    fn preview(
        ui:     &mut egui::Ui,
        editor: &Editor,
        theme:  &GuiTheme,
        state:  &mut CommitState,
    ) {
        let header = match &state.selected {
            Some(p) => format!("Diff — {}", p.rsplit('/').next().unwrap_or(p)),
            None    => "Diff".to_string(),
        };
        ui.horizontal(|ui| {
            ui.checkbox(&mut state.show_diff, RichText::new(header).size(11.0).color(theme.overlay0));
        });
        if !state.show_diff { return }

        let Some(path) = state.selected.clone() else {
            ui.label(RichText::new("Select a file to see its diff.").size(FONT_SIZE).color(theme.overlay0));
            return;
        };

        let mono = FontId::new(FONT_SIZE, FontFamily::Monospace);
        let text = editor.vc.preview.as_deref()
            .filter(|_| editor.vc.preview_path.as_deref() == Some(path.as_str()))
            .unwrap_or("(no diff loaded)");
        egui::ScrollArea::vertical()
            .id_salt("commit_diff")
            .auto_shrink([false, false])
            .max_height(240.0)
            .show(ui, |ui| {
                for line in text.lines() {
                    let color = match line.as_bytes().first() {
                        Some(b'+') if !line.starts_with("+++") => theme.green,
                        Some(b'-') if !line.starts_with("---") => theme.red,
                        Some(b'@') => theme.blue,
                        _ => theme.subtext1,
                    };
                    ui.label(RichText::new(line).font(mono.clone()).color(color));
                }
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn section(
        ui:           &mut egui::Ui,
        theme:        &GuiTheme,
        title:        &str,
        entries:      &[(String, char)],
        staged:       bool,
        state:        &mut CommitState,
        actions:      &mut Vec<CommitAction>,
        diff_clicked: &mut Option<String>,
        conflicts:    bool,
    ) {
        if entries.is_empty() { return }
        ui.add_space(4.0);
        ui.label(RichText::new(format!("{} ({})", title, entries.len()))
            .size(11.0).color(theme.overlay0).strong());
        for (path, code) in entries {
            let sel = state.selected.as_deref() == Some(path.as_str());
            let (rect, resp) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), ROW_H), Sense::click());
            if sel {
                ui.painter().rect_filled(rect, 2, theme.surface0);
            } else if resp.hovered() {
                ui.painter().rect_filled(rect, 2, theme.surface0.gamma_multiply(0.5));
            }
            let cy = rect.center().y;
            let code_color = match code {
                'A' | '?' => theme.green,
                'D' => theme.red,
                '!' => theme.warning,
                _ => theme.lavender,
            };
            ui.painter().text(egui::Pos2::new(rect.left() + 8.0, cy),
                egui::Align2::LEFT_CENTER, code.to_string(),
                FontId::new(FONT_SIZE, FontFamily::Monospace), code_color);
            ui.painter().text(egui::Pos2::new(rect.left() + 26.0, cy),
                egui::Align2::LEFT_CENTER, path,
                FontId::new(FONT_SIZE, FontFamily::Proportional), theme.subtext1);

            // Stage/unstage chevron on the right.
            let btn = egui::Rect::from_center_size(
                egui::Pos2::new(rect.right() - 14.0, cy), Vec2::splat(18.0));
            let btn_resp = ui.interact(btn, egui::Id::new(("commit_stage", path, staged)), Sense::click())
                .on_hover_text(if staged { "Unstage" } else { "Stage" });
            ui.painter().text(btn.center(), egui::Align2::CENTER_CENTER,
                if staged { "▲" } else { "▼" }, FontId::new(12.0, FontFamily::Proportional),
                if btn_resp.hovered() { theme.blue } else { theme.overlay0 });
            if btn_resp.clicked() {
                if conflicts {
                    actions.push(CommitAction::Stage(path.clone()));
                } else if staged {
                    actions.push(CommitAction::Unstage(path.clone()));
                } else {
                    actions.push(CommitAction::Stage(path.clone()));
                }
            }

            if resp.clicked() { *diff_clicked = Some(path.clone()); }
            resp.context_menu(|ui| {
                if ui.button("Show Diff").clicked() {
                    actions.push(CommitAction::ShowDiff(path.clone()));
                    ui.close();
                }
                if ui.button("Open in Editor").clicked() {
                    actions.push(CommitAction::OpenFile(path.clone()));
                    ui.close();
                }
                if ui.button("Copy Path").clicked() {
                    actions.push(CommitAction::CopyPath(path.clone()));
                    ui.close();
                }
                ui.separator();
                if !staged && ui.button(RichText::new("Rollback…").color(theme.red)).clicked() {
                    actions.push(CommitAction::Discard(path.clone()));
                    ui.close();
                }
            });
        }
    }
}

fn status_codes(files: &[crate::kernel::vc::FileStatus]) -> Vec<(String, char)> {
    files.iter().map(|f| (f.path.clone(), f.code)).collect()
}
