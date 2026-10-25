//! Commit tool window — section bodies: the change lists and the stash list.
//!
//! Split from `gui_commit_tool` at the layout boundary so each file stays a
//! single abstraction.  Methods here are `impl CommitPanel` bodies the spine
//! calls; nothing here mutates the editor.  The side-by-side diff lives in
//! the editor (`vc_diff`), not here.

use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Pos2, Rect, RichText, Sense, Vec2};

use crate::kernel::state::Editor;
use crate::kernel::vc::{FileStatus, StashEntry, VcStatus};

use super::gui_commit_tool::{
    section_header, CommitAction, CommitPanel, CommitState, FONT_SIZE, ROW_H, SMALL,
};
use super::gui_theme::GuiTheme;

impl CommitPanel {
    /// The four change lists.  Returns the path a row click selected.
    pub(super) fn changes(
        ui:      &mut egui::Ui,
        theme:   &GuiTheme,
        status:  &VcStatus,
        state:   &mut CommitState,
        actions: &mut Vec<CommitAction>,
    ) -> Option<String> {
        let conflicts: Vec<(String, char)> =
            status.conflicts.iter().map(|p| (p.clone(), '!')).collect();
        let staged    = status_codes(&status.staged);
        let unstaged  = status_codes(&status.unstaged);
        let untracked: Vec<(String, char)> =
            status.untracked.iter().map(|p| (p.clone(), '?')).collect();

        let mut clicked = None;
        egui::ScrollArea::vertical()
            .id_salt("commit_changes")
            .auto_shrink([false, false])
            .max_height(260.0)
            .show(ui, |ui| {
                for (title, entries, staged_side, is_conflict) in [
                    ("Conflicts", &conflicts, false, true),
                    ("Staged",    &staged,    true,  false),
                    ("Unstaged",  &unstaged,  false, false),
                    ("Untracked", &untracked, false, false),
                ] {
                    if entries.is_empty() { continue; }
                    ui.add_space(3.0);
                    group_label(ui, theme, title, entries.len());
                    for (path, code) in entries {
                        let selected = state.selected.as_deref() == Some(path.as_str());
                        let req = change_row(
                            ui, theme, path, *code, selected, staged_side, is_conflict, actions);
                        if req.toggle {
                            if is_conflict || !staged_side {
                                actions.push(CommitAction::Stage(path.clone()));
                            } else {
                                actions.push(CommitAction::Unstage(path.clone()));
                            }
                        }
                        if req.clicked {
                            clicked = Some(path.clone());
                        }
                    }
                }
                if conflicts.is_empty() && staged.is_empty()
                    && unstaged.is_empty() && untracked.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Working tree clean")
                        .color(theme.overlay0).size(SMALL));
                }
            });
        clicked
    }

    /// Stash list + the "stash now" field.
    pub(super) fn stashes(
        ui:      &mut egui::Ui,
        editor:  &Editor,
        theme:   &GuiTheme,
        state:   &mut CommitState,
        actions: &mut Vec<CommitAction>,
    ) {
        let count = editor.vc.stashes.len();
        let mut open = state.show_stashes;
        section_header(ui, theme, "Stashes", count, &mut open);
        state.show_stashes = open;
        if !open { return }

        ui.horizontal(|ui| {
            let resp = ui.add(egui::TextEdit::singleline(&mut state.new_stash)
                .hint_text(RichText::new("Stash message…").size(SMALL))
                .desired_width((ui.available_width() - 58.0).max(60.0)));
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter || ui.small_button(RichText::new("Stash").size(SMALL)).clicked() {
                actions.push(CommitAction::StashPush(state.new_stash.trim().to_string()));
                state.new_stash.clear();
            }
        });

        if editor.vc.stashes.is_empty() {
            ui.label(RichText::new("No stashes").color(theme.overlay0).size(SMALL));
            return;
        }
        for StashEntry { index, message } in &editor.vc.stashes {
            stash_row(ui, theme, *index, message, actions);
        }
    }
}

// ── Row builders ─────────────────────────────────────────────────────────────

/// The colour a status code reads in.  Shape carries meaning too (the letter
/// itself), so colour is never the only signal.
fn status_color(theme: &GuiTheme, code: char) -> Color32 {
    match code {
        'A' | '?' => theme.green,
        'D'       => theme.red,
        '!'       => theme.warning,
        _         => theme.blue,
    }
}

/// What a change row's controls asked for.
struct RowRequest {
    clicked: bool,
    toggle:  bool,
}

/// One change row: status letter, path (dirs dimmed), a stage/unstage chevron
/// that only lights up on hover, and a right-click menu.  The chevron is
/// hit-tested separately so the row click and the button click never collide.
#[allow(clippy::too_many_arguments)]
fn change_row(
    ui:        &mut egui::Ui,
    theme:     &GuiTheme,
    path:      &str,
    code:      char,
    selected:  bool,
    staged:    bool,
    conflicts: bool,
    actions:   &mut Vec<CommitAction>,
) -> RowRequest {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ROW_H), Sense::click());
    let clicked = resp.clicked();
    if selected {
        ui.painter().rect_filled(rect, 2, theme.surface0);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 2, theme.surface0.gamma_multiply(0.45));
    }
    let cy = rect.center().y;

    ui.painter().text(Pos2::new(rect.left() + 6.0, cy), Align2::LEFT_CENTER,
        code.to_string(), FontId::new(FONT_SIZE, FontFamily::Monospace), status_color(theme, code));

    let (dir, name) = match path.rsplit_once('/') {
        Some((d, n)) => (format!("{d}/"), n.to_string()),
        None => (String::new(), path.to_string()),
    };
    let name_x = rect.left() + 22.0;
    let dir_w = if dir.is_empty() { 0.0 } else {
        ui.painter().layout_no_wrap(dir.clone(),
            FontId::new(FONT_SIZE, FontFamily::Proportional), theme.overlay0).size().x
    };
    if !dir.is_empty() {
        ui.painter().text(Pos2::new(name_x, cy), Align2::LEFT_CENTER, dir,
            FontId::new(FONT_SIZE, FontFamily::Proportional), theme.overlay0);
    }
    ui.painter().text(Pos2::new(name_x + dir_w, cy), Align2::LEFT_CENTER, name,
        FontId::new(FONT_SIZE, FontFamily::Proportional), theme.subtext1);

    // Chevron button, right-aligned.
    let btn = Rect::from_center_size(Pos2::new(rect.right() - 12.0, cy), Vec2::splat(18.0));
    let btn_resp = ui.interact(btn, egui::Id::new(("commit_toggle", path)), Sense::click())
        .on_hover_text(if conflicts { "Mark resolved" } else if staged { "Unstage" } else { "Stage" });
    let glyph = if staged { "▲" } else { "▼" };
    ui.painter().text(btn.center(), Align2::CENTER_CENTER, glyph,
        FontId::new(12.0, FontFamily::Proportional),
        if btn_resp.hovered() { theme.blue } else { theme.overlay0 });

    resp.context_menu(|ui| {
        if conflicts {
            if ui.button("Accept yours").clicked() {
                actions.push(CommitAction::TakeOurs(path.to_string()));
                ui.close();
            }
            if ui.button("Accept theirs").clicked() {
                actions.push(CommitAction::TakeTheirs(path.to_string()));
                ui.close();
            }
            ui.separator();
        }
        if ui.button("Show diff").clicked() {
            actions.push(CommitAction::ShowDiff(path.to_string()));
            ui.close();
        }
        if ui.button("Open in editor").clicked() {
            actions.push(CommitAction::OpenFile(path.to_string()));
            ui.close();
        }
        if ui.button("Copy path").clicked() {
            actions.push(CommitAction::CopyPath(path.to_string()));
            ui.close();
        }
        if !staged && !conflicts {
            ui.separator();
            if ui.button(RichText::new("Rollback…").color(theme.red)).clicked() {
                actions.push(CommitAction::Discard(path.to_string()));
                ui.close();
            }
        }
    });

    RowRequest { clicked, toggle: btn_resp.clicked() }
}

/// A group label ("Staged  2") above a run of rows.
fn group_label(ui: &mut egui::Ui, theme: &GuiTheme, title: &str, count: usize) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
    ui.painter().text(Pos2::new(rect.left() + 4.0, rect.center().y), Align2::LEFT_CENTER,
        format!("{title}  {count}"),
        FontId::new(SMALL, FontFamily::Proportional), theme.subtext0);
}

/// One stash row: `stash@{n}  message` with pop/apply/drop buttons.
fn stash_row(
    ui:      &mut egui::Ui,
    theme:   &GuiTheme,
    index:   usize,
    message: &str,
    actions: &mut Vec<CommitAction>,
) {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ROW_H), Sense::hover());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 2, theme.surface0.gamma_multiply(0.4));
    }
    let cy = rect.center().y;
    ui.painter().text(Pos2::new(rect.left() + 6.0, cy), Align2::LEFT_CENTER,
        format!("stash@{{{index}}}"),
        FontId::new(SMALL, FontFamily::Monospace), theme.lavender);
    ui.painter().text(Pos2::new(rect.left() + 62.0, cy), Align2::LEFT_CENTER,
        truncate(message, 24),
        FontId::new(SMALL, FontFamily::Proportional), theme.subtext0);

    let mut x = rect.right() - 4.0;
    for (glyph, tip, color, action) in [
        ("✕", "Drop",  theme.red,   CommitAction::StashDrop(index)),
        ("↧", "Apply", theme.blue,  CommitAction::StashApply(index)),
        ("⇪", "Pop",   theme.green, CommitAction::StashPop(index)),
    ] {
        let btn = egui::Rect::from_center_size(Pos2::new(x - 10.0, cy), Vec2::splat(18.0));
        let br = ui.interact(btn, egui::Id::new(("stash_btn", index, glyph)), Sense::click())
            .on_hover_text(tip);
        if br.hovered() {
            ui.painter().rect_filled(btn, 3, theme.surface1);
        }
        ui.painter().text(btn.center(), Align2::CENTER_CENTER, glyph,
            FontId::new(12.0, FontFamily::Proportional),
            if br.hovered() { color } else { theme.overlay0 });
        if br.clicked() { actions.push(action); }
        x -= 20.0;
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn status_codes(files: &[FileStatus]) -> Vec<(String, char)> {
    files.iter().map(|f| (f.path.clone(), f.code)).collect()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max { return s.to_string() }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
