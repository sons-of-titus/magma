//! Debug tool window — the DAP surface, laid out the way JetBrains' Debug tool
//! window is: an icon-only step-control toolbar over Frames / Threads / Console
//! / Watches / Breakpoints tabs.  The Frames tab pairs the call stack with the
//! variables tree of the selected frame.

use eframe::egui::{self, FontFamily, FontId, RichText};
use magma::kernel::state::Editor;
use super::gui_run_tool::toolbar_button;
use super::gui_theme::{GuiTheme, icons};
use super::tool_window::{ToolAction, ToolWindow};

fn mono(size: f32) -> FontId { FontId::new(size, FontFamily::Monospace) }
fn prop(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

/// The Debug tool window's tabs, in JetBrains order.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum DebugTab {
    #[default]
    Frames,
    Threads,
    Console,
    Watches,
    Breakpoints,
}

const TABS: &[(DebugTab, &str)] = &[
    (DebugTab::Frames, "Frames"),
    (DebugTab::Threads, "Threads"),
    (DebugTab::Console, "Console"),
    (DebugTab::Watches, "Watches"),
    (DebugTab::Breakpoints, "Breakpoints"),
];

pub struct DebugToolWindow {
    watch_input: String,
    eval_input: String,
    tab:        DebugTab,
}

impl DebugToolWindow {
    pub fn new() -> Self {
        Self { watch_input: String::new(), eval_input: String::new(), tab: DebugTab::Frames }
    }
}

fn section_label(ui: &mut egui::Ui, theme: &GuiTheme, text: &str) {
    ui.label(RichText::new(text).font(prop(10.0)).color(theme.overlay0).strong());
}

fn hint(ui: &mut egui::Ui, theme: &GuiTheme, text: &str) {
    ui.label(RichText::new(text).font(prop(12.0)).color(theme.overlay0));
}

impl ToolWindow for DebugToolWindow {
    fn title(&self) -> &str { "Debug" }
    fn icon(&self)  -> char { icons::DEBUG.chars().next().unwrap_or(' ') }

    fn ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        let mut actions = Vec::new();
        let session = editor.debug.active_session().cloned();
        let stopped = session.as_ref().map(|s| s.stopped).unwrap_or(false);
        let live = session.is_some();
        let breakpoints: Vec<(String, usize, bool)> = editor.debug.all_breakpoints()
            .iter().map(|bp| (bp.file.clone(), bp.line, bp.enabled)).collect();

        // ── Session status ────────────────────────────────────────────────
        ui.horizontal(|ui| {
            let (state, color) = match (&session, stopped) {
                (None, _)       => ("no session".to_string(),           theme.overlay0),
                (Some(s), false) => (format!("{} · Running", s.adapter), theme.green),
                (Some(s), true)  => (format!("{} · Paused", s.adapter),  theme.red),
            };
            ui.label(RichText::new(state).font(prop(12.0)).color(color).strong());
        });

        // ── Toolbar (icon-only, JetBrains order) ──────────────────────────
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if toolbar_button(ui, icons::RESTART, live, theme.subtext0, "Rerun", theme) {
                actions.push(ToolAction::DebugCmd("dbg-restart"));
            }
            if toolbar_button(ui, icons::RUN, live && stopped, theme.green, "Resume", theme) {
                actions.push(ToolAction::DebugCmd("dbg-continue"));
            }
            if toolbar_button(ui, icons::PAUSE, live && !stopped, theme.subtext0, "Pause", theme) {
                actions.push(ToolAction::DebugCmd("dbg-pause"));
            }
            if toolbar_button(ui, icons::STOP, live, theme.red, "Stop", theme) {
                actions.push(ToolAction::DebugCmd("dbg-stop"));
            }
            ui.add_space(10.0);
            if toolbar_button(ui, icons::STEP_OVER, live && stopped, theme.subtext0, "Step Over", theme) {
                actions.push(ToolAction::DebugCmd("dbg-step-over"));
            }
            if toolbar_button(ui, icons::STEP_IN, live && stopped, theme.subtext0, "Step Into", theme) {
                actions.push(ToolAction::DebugCmd("dbg-step-in"));
            }
            if toolbar_button(ui, icons::STEP_OUT, live && stopped, theme.subtext0, "Step Out", theme) {
                actions.push(ToolAction::DebugCmd("dbg-step-out"));
            }
        });

        // ── Tabs ──────────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (tab, label) in TABS {
                if ui.selectable_label(self.tab == *tab, *label).clicked() {
                    self.tab = *tab;
                }
            }
        });
        ui.separator();

        match self.tab {
            // Call stack beside the variables of the selected frame.
            DebugTab::Frames => match &session {
                Some(s) if stopped => {
                    if s.frames.is_empty() {
                        hint(ui, theme, "no frames");
                    } else {
                        section_label(ui, theme, "CALL STACK");
                        for f in &s.frames {
                            let short = f.source.as_deref()
                                .map(|p| p.rsplit('/').next().unwrap_or(p).to_string())
                                .unwrap_or_else(|| "?".to_string());
                            let label = RichText::new(format!("{}  {}:{}:{}", f.name, short, f.line, f.column))
                                .font(mono(12.0)).color(theme.text);
                            if ui.add(egui::Button::new(label).frame(false)).clicked()
                                && let Some(path) = &f.source {
                                actions.push(ToolAction::Goto { path: path.clone(), line: f.line });
                            }
                        }
                    }
                    ui.add_space(6.0);
                    section_label(ui, theme, "VARIABLES");
                    match s.scope_ref.and_then(|r| s.variable_scopes.get(&r)) {
                        Some(vars) if !vars.is_empty() =>
                            render_variable_rows(ui, theme, s, vars, 0, &mut actions),
                        _ => hint(ui, theme, "no variables"),
                    }
                }
                Some(_) => hint(ui, theme, "running — pause to inspect frames"),
                None     => hint(ui, theme, "no session"),
            },

            DebugTab::Threads => match &session {
                Some(s) => {
                    if s.threads.is_empty() {
                        hint(ui, theme, "no threads");
                    }
                    for t in &s.threads {
                        let current = s.stopped_thread == Some(t.id);
                        let marker = if current { icons::CARET_RIGHT } else { " " };
                        ui.label(RichText::new(format!("{marker} {}", t.name))
                            .font(prop(12.0))
                            .color(if current { theme.text } else { theme.subtext1 }));
                    }
                }
                None => hint(ui, theme, "no session"),
            },

            DebugTab::Console => match &session {
                Some(s) => {
                    egui::ScrollArea::vertical()
                        .id_salt("magma_debug_console")
                        .auto_shrink([false, false])
                        .max_height(260.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            if s.console.is_empty() {
                                hint(ui, theme, "console is empty");
                            }
                            for line in &s.console {
                                ui.label(RichText::new(line.clone()).font(mono(11.0)).color(theme.subtext1));
                            }
                        });
                    ui.horizontal(|ui| {
                        let resp = ui.add(egui::TextEdit::singleline(&mut self.eval_input)
                            .hint_text("evaluate…")
                            .font(mono(12.0))
                            .desired_width(f32::INFINITY));
                        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter)) && resp.has_focus();
                        if (ui.button("Eval").clicked() || (enter && !self.eval_input.trim().is_empty()))
                            && !self.eval_input.trim().is_empty() {
                            actions.push(ToolAction::DebugEval(self.eval_input.trim().to_string()));
                            self.eval_input.clear();
                        }
                    });
                }
                None => hint(ui, theme, "no session"),
            },

            DebugTab::Watches => {
                let watches = editor.debug.watches.clone();
                if watches.is_empty() {
                    hint(ui, theme, "no watches");
                }
                for (i, expr) in watches.iter().enumerate() {
                    let result = session.as_ref()
                        .and_then(|s| s.watch_results.get(i).cloned().flatten());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(expr.clone()).font(mono(12.0)).color(theme.lavender));
                        match result {
                            Some(v) => ui.label(RichText::new(format!("= {v}")).font(mono(12.0)).color(theme.text)),
                            None    => ui.label(RichText::new("—").font(mono(12.0)).color(theme.overlay0)),
                        };
                        if ui.small_button("×").clicked() {
                            actions.push(ToolAction::WatchRemove(expr.clone()));
                        }
                    });
                }
                ui.horizontal(|ui| {
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.watch_input)
                        .hint_text("new watch expression")
                        .font(mono(12.0))
                        .desired_width(180.0));
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter)) && resp.has_focus();
                    if (ui.button("Add").clicked() || (enter && !self.watch_input.trim().is_empty()))
                        && !self.watch_input.trim().is_empty() {
                        actions.push(ToolAction::WatchAdd(self.watch_input.trim().to_string()));
                        self.watch_input.clear();
                    }
                });
            }

            DebugTab::Breakpoints => {
                if breakpoints.is_empty() {
                    hint(ui, theme, "no breakpoints");
                }
                for (file, line, enabled) in &breakpoints {
                    ui.horizontal(|ui| {
                        let mut checked = *enabled;
                        if ui.checkbox(&mut checked, "").changed() && checked != *enabled {
                            actions.push(ToolAction::BreakpointToggle { file: file.clone(), line: *line });
                        }
                        let short = file.rsplit('/').next().unwrap_or(file);
                        let label = RichText::new(format!("{short}:{line}"))
                            .font(mono(12.0))
                            .color(if *enabled { theme.text } else { theme.overlay0 });
                        if ui.add(egui::Button::new(label).frame(false)).clicked() {
                            actions.push(ToolAction::Goto { path: file.clone(), line: *line });
                        }
                        if ui.small_button("×").clicked() {
                            actions.push(ToolAction::BreakpointRemove { file: file.clone(), line: *line });
                        }
                    });
                }
            }
        }

        actions
    }
}

fn render_variable_rows(
    ui: &mut egui::Ui,
    theme: &GuiTheme,
    session: &magma::kernel::debug::DebugSession,
    vars: &[magma::kernel::debug::Variable],
    depth: usize,
    actions: &mut Vec<ToolAction>,
) {
    for var in vars {
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 14.0);
            let expandable = var.variables_reference > 0;
            let fetched = session.variable_scopes.contains_key(&var.variables_reference);
            let marker = if !expandable { " " }
                else if fetched { icons::CARET_DOWN } else { icons::CARET_RIGHT };
            let name = RichText::new(format!("{marker} {}", var.name))
                .font(mono(12.0)).color(theme.lavender);
            if expandable {
                if ui.add(egui::Button::new(name).frame(false)).clicked() && !fetched {
                    actions.push(ToolAction::DebugExpand(var.variables_reference));
                }
            } else {
                ui.label(name);
            }
            let value = RichText::new(format!("= {}{}",
                    var.value,
                    var.type_name.as_deref().map(|t| format!(" ({t})")).unwrap_or_default()))
                .font(mono(12.0)).color(theme.text);
            ui.label(value);
        });
        if let Some(children) = session.variable_scopes.get(&var.variables_reference) {
            render_variable_rows(ui, theme, session, children, depth + 1, actions);
        }
    }
}
