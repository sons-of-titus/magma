//! Commit tool window — the JetBrains-style Version Control panel.
//!
//! One surface for the whole change lifecycle: the change lists with
//! click-to-stage, a branch widget, the commit box, a per-hunk stage/discard
//! strip, and the stash list.  It reads `editor.vc` and returns
//! `CommitAction`s the shell applies at its write boundary — the widget never
//! mutates the editor itself.
//!
//! Layout, section bodies, and rows live in `gui_commit_sections`; this file
//! owns the action/state contract and the panel's spine (header, branch row,
//! commit box).

use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Pos2, RichText, Sense, Vec2};

use kernel::state::Editor;
use kernel::vc::VcStatus;

use super::gui_theme::{icons, GuiTheme};

pub(super) const ROW_H:    f32 = 22.0;
pub(super) const FONT_SIZE: f32 = 12.5;
pub(super) const SMALL:     f32 = 11.0;

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
    StashPush(String),
    StashPop(usize),
    StashApply(usize),
    StashDrop(usize),
    Fetch,
    Pull,
    Push,
    Log,
    Blame,
    TakeOurs(String),
    TakeTheirs(String),
}

/// Commit-window UI state (lives on `GuiApp`, like `SidebarState`).
#[derive(Default)]
pub struct CommitState {
    pub message: String,
    pub amend: bool,
    pub selected: Option<String>,
    pub new_branch: String,
    pub new_stash: String,
    pub show_stashes: bool,
    /// Path the diff was last requested for — avoids re-requesting each frame.
    diff_for: Option<String>,
    /// Whether the branch list was already fetched for this repo.
    branches_for: Option<String>,
    /// Whether the branch-list fetch was dispatched (one per repo, so an
    /// empty list does not re-spawn a `git branch` every frame).
    branches_requested: bool,
}

impl CommitState {
    /// Reset per-repo state when the repository root changes.
    pub fn sync_root(&mut self, root: Option<&str>) {
        if self.branches_for.as_deref() != root {
            self.branches_for = root.map(str::to_string);
            self.branches_requested = false;
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
            Self::empty_state(ui, theme, &mut actions);
            return actions;
        };

        // Fetch branches once per repo.
        if !state.branches_requested {
            state.branches_requested = true;
            actions.push(CommitAction::BranchList);
        }

        Self::header(ui, theme, status, &mut actions);
        Self::toolbar(ui, theme, &mut actions);
        Self::branch_row(ui, editor, status, theme, state, &mut actions);
        separator(ui, theme);

        // Change lists — selecting a row opens its diff in the editor tab.
        if let Some(path) = Self::changes(ui, theme, status, state, &mut actions) {
            state.selected = Some(path.clone());
            if state.diff_for.as_deref() != Some(path.as_str()) {
                state.diff_for = Some(path.clone());
                actions.push(CommitAction::ShowDiff(path));
            }
        }

        separator(ui, theme);
        Self::commit_box(ui, theme, state, &mut actions);

        separator(ui, theme);
        Self::stashes(ui, editor, theme, state, &mut actions);

        actions
    }

    /// The no-repository-yet state.  It teaches the space and offers the one
    /// action that changes it.
    fn empty_state(ui: &mut egui::Ui, theme: &GuiTheme, actions: &mut Vec<CommitAction>) {
        ui.add_space(18.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("No changes to show")
                .color(theme.subtext0).size(FONT_SIZE));
            ui.add_space(4.0);
            ui.label(RichText::new("Open a file in a Git repository")
                .color(theme.overlay0).size(SMALL));
            ui.label(RichText::new("to review and commit its changes.")
                .color(theme.overlay0).size(SMALL));
            ui.add_space(10.0);
            if ui.button(RichText::new("Refresh").size(FONT_SIZE)).clicked() {
                actions.push(CommitAction::Refresh);
            }
        });
    }

    /// Branch head + backend, the panel's identity row.
    fn header(ui: &mut egui::Ui, theme: &GuiTheme, status: &VcStatus, _actions: &mut Vec<CommitAction>) {
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            ui.label(RichText::new(&status.branch).strong().color(theme.text).size(FONT_SIZE + 0.5));
            if !status.backend_name.is_empty() {
                ui.label(RichText::new(&status.backend_name).color(theme.overlay0).size(SMALL));
            }
        });
    }

    /// The Version Control toolbar — every repository-level action, on a
    /// button, so nothing here is colon-command-only.
    fn toolbar(ui: &mut egui::Ui, theme: &GuiTheme, actions: &mut Vec<CommitAction>) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(2.0);
            for (glyph, tip, action) in [
                (icons::REFRESH, "Refresh", CommitAction::Refresh),
                (icons::FETCH,   "Fetch",   CommitAction::Fetch),
                (icons::PULL,    "Pull",    CommitAction::Pull),
                (icons::PUSH,    "Push",    CommitAction::Push),
                (icons::LOG,     "Log",     CommitAction::Log),
                (icons::BLAME,   "Blame",   CommitAction::Blame),
            ] {
                if ghost_button(ui, glyph, theme, theme.overlay0, tip) {
                    actions.push(action);
                }
            }
        });
    }

    /// Branch combo + inline new-branch field.
    fn branch_row(
        ui: &mut egui::Ui,
        editor: &Editor,
        status: &VcStatus,
        theme: &GuiTheme,
        state: &mut CommitState,
        actions: &mut Vec<CommitAction>,
    ) {
        ui.horizontal(|ui| {
            let mut chosen = status.branch.clone();
            egui::ComboBox::from_id_salt("commit_branch")
                .selected_text(RichText::new(chosen.clone()).size(FONT_SIZE))
                .width(ui.available_width() - 6.0)
                .show_ui(ui, |ui| {
                    for b in &editor.vc.branches {
                        ui.selectable_value(&mut chosen, b.clone(), b);
                    }
                    if editor.vc.branches.is_empty() {
                        ui.label(RichText::new("no branches").size(SMALL).color(theme.overlay0));
                    }
                });
            if chosen != status.branch {
                actions.push(CommitAction::Checkout(chosen));
            }
        });
        let resp = ui.add(egui::TextEdit::singleline(&mut state.new_branch)
            .hint_text(RichText::new("New branch…").size(SMALL))
            .desired_width(f32::INFINITY));
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if enter && !state.new_branch.trim().is_empty() {
            actions.push(CommitAction::NewBranch(state.new_branch.trim().to_string()));
            state.new_branch.clear();
        }
    }

    /// Commit message + amend toggle + the primary action.
    fn commit_box(ui: &mut egui::Ui, theme: &GuiTheme, state: &mut CommitState, actions: &mut Vec<CommitAction>) {
        ui.add_space(2.0);
        ui.label(RichText::new("Commit message").size(SMALL).color(theme.overlay0));
        ui.add(egui::TextEdit::multiline(&mut state.message)
            .hint_text(RichText::new("Describe the change…").size(FONT_SIZE))
            .desired_rows(3)
            .desired_width(f32::INFINITY));
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut state.amend, RichText::new("Amend").size(SMALL));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let enabled = !state.message.trim().is_empty() || state.amend;
                let label = if state.amend { "Amend" } else { "Commit" };
                if ui.add_enabled(enabled, egui::Button::new(
                    RichText::new(label).size(FONT_SIZE).strong()))
                    .clicked() {
                    actions.push(CommitAction::Commit {
                        message: state.message.clone(),
                        amend: state.amend,
                    });
                    state.message.clear();
                    state.amend = false;
                }
            });
        });
    }
}

// ── Shared chrome ────────────────────────────────────────────────────────────

/// A 1px hairline between sections.
pub(super) fn separator(ui: &mut egui::Ui, theme: &GuiTheme) {
    ui.add_space(3.0);
    let rect = ui.available_rect_before_wrap();
    ui.painter().hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0, theme.border));
    ui.add_space(4.0);
}

/// A borderless square glyph button that fills on hover — JetBrains' toolbar
/// button.  Returns true when clicked.
pub(super) fn ghost_button(ui: &mut egui::Ui, glyph: &str, theme: &GuiTheme, color: Color32, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
    let resp = resp.on_hover_text(tip);
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4, theme.surface0);
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, glyph,
        FontId::new(13.0, FontFamily::Proportional), if resp.hovered() { theme.text } else { color });
    resp.clicked()
}

/// A collapsible section header: chevron, title, count pill.  Returns true
/// when the header itself was clicked (toggle).
pub(super) fn section_header(
    ui:      &mut egui::Ui,
    theme:   &GuiTheme,
    title:   &str,
    count:   usize,
    open:    &mut bool,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 20.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 3, theme.surface0.gamma_multiply(0.5));
    }
    let cy = rect.center().y;
    ui.painter().text(Pos2::new(rect.left() + 2.0, cy), Align2::LEFT_CENTER,
        if *open { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT },
        FontId::new(11.0, FontFamily::Proportional), theme.overlay0);
    ui.painter().text(Pos2::new(rect.left() + 16.0, cy), Align2::LEFT_CENTER,
        title, FontId::new(SMALL, FontFamily::Proportional), theme.subtext1);
    if count > 0 {
        let count_text = count.to_string();
        let cw = ui.painter().layout_no_wrap(count_text.clone(),
            FontId::new(10.0, FontFamily::Proportional), theme.overlay0).size().x;
        ui.painter().text(Pos2::new(rect.right() - cw - 2.0, cy), Align2::LEFT_CENTER,
            count_text, FontId::new(10.0, FontFamily::Proportional), theme.overlay0);
    }
    if resp.clicked() {
        *open = !*open;
        return true;
    }
    false
}

