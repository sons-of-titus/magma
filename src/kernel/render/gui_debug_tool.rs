//! Debug tool window — the real DAP surface: session controls, breakpoints,
//! threads, the call stack (click → open at the frame's line), an expandable
//! variables tree, watches, and the adapter console.

use eframe::egui::{self, FontFamily, FontId, RichText};
use crate::kernel::state::Editor;
use super::gui_theme::GuiTheme;
use super::tool_window::{ToolAction, ToolWindow};

fn mono(size: f32) -> FontId { FontId::new(size, FontFamily::Monospace) }
fn prop(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

pub struct DebugToolWindow {
    watch_input: String,
    eval_input: String,
}

impl DebugToolWindow {
    pub fn new() -> Self {
        Self { watch_input: String::new(), eval_input: String::new() }
    }
}

fn section_label(ui: &mut egui::Ui, theme: &GuiTheme, text: &str) {
    ui.label(RichText::new(text).font(prop(10.0)).color(theme.overlay0).strong());
}

impl ToolWindow for DebugToolWindow {
    fn title(&self) -> &str { "Debug" }
    fn icon(&self)  -> char { '֍' }

    fn ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        let mut actions = Vec::new();
        let session = editor.debug.active_session().cloned();
        let stopped = session.as_ref().map(|s| s.stopped).unwrap_or(false);

        // ── Session toolbar ───────────────────────────────────────────────
        ui.horizontal(|ui| {
            let state = match (&session, stopped) {
                (None, _) => "no session — :dbg-start or :debug-config".to_string(),
                (Some(s), false) => format!("{} · RUNNING", s.adapter),
                (Some(s), true) => format!("{} · STOPPED", s.adapter),
            };
            let color = if stopped { theme.red } else { theme.green };
            ui.label(RichText::new(state).font(prop(12.0)).color(color).strong());
        });
        ui.horizontal(|ui| {
            let live = session.is_some();
            let (play, pause) = if stopped { ("▶ Continue", "⏸ Pause") } else { ("▶ Continue", "⏸ Pause") };
            if ui.add_enabled(live && stopped, egui::Button::new(play))
                .on_hover_text("dbg-continue").clicked() {
                actions.push(ToolAction::DebugCmd("dbg-continue"));
            }
            if ui.add_enabled(live && !stopped, egui::Button::new(pause))
                .on_hover_text("dbg-pause").clicked() {
                actions.push(ToolAction::DebugCmd("dbg-pause"));
            }
            if ui.add_enabled(live && stopped, egui::Button::new("⤵ Step Over"))
                .on_hover_text("dbg-step-over").clicked() {
                actions.push(ToolAction::DebugCmd("dbg-step-over"));
            }
            if ui.add_enabled(live && stopped, egui::Button::new("↓ Step In"))
                .on_hover_text("dbg-step-in").clicked() {
                actions.push(ToolAction::DebugCmd("dbg-step-in"));
            }
            if ui.add_enabled(live && stopped, egui::Button::new("↑ Step Out"))
                .on_hover_text("dbg-step-out").clicked() {
                actions.push(ToolAction::DebugCmd("dbg-step-out"));
            }
            if ui.add_enabled(live, egui::Button::new("↻"))
                .on_hover_text("dbg-restart").clicked() {
                actions.push(ToolAction::DebugCmd("dbg-restart"));
            }
            if ui.add_enabled(live, egui::Button::new("■"))
                .on_hover_text("dbg-stop").clicked() {
                actions.push(ToolAction::DebugCmd("dbg-stop"));
            }
        });
        ui.separator();

        // ── Breakpoints (project-wide, session or not) ────────────────────
        let bps: Vec<(String, usize, bool)> = editor.debug.all_breakpoints()
            .iter().map(|bp| (bp.file.clone(), bp.line, bp.enabled)).collect();
        if !bps.is_empty() {
            section_label(ui, theme, "BREAKPOINTS");
            let short = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
            for (file, line, enabled) in bps {
                ui.horizontal(|ui| {
                    let mut checked = enabled;
                    if ui.checkbox(&mut checked, "").changed() && checked != enabled {
                        actions.push(ToolAction::BreakpointToggle { file: file.clone(), line });
                    }
                    let label = RichText::new(format!("{}:{}", short(&file), line))
                        .font(mono(12.0))
                        .color(if enabled { theme.text } else { theme.overlay0 });
                    if ui.add(egui::Button::new(label).frame(false)).clicked() {
                        actions.push(ToolAction::Goto { path: file.clone(), line });
                    }
                    if ui.small_button("×").clicked() {
                        actions.push(ToolAction::BreakpointRemove { file: file.clone(), line });
                    }
                });
            }
            ui.add_space(2.0);
        }

        let Some(session) = session else { return actions };

        // ── Threads ───────────────────────────────────────────────────────
        if !session.threads.is_empty() {
            section_label(ui, theme, "THREADS");
            for t in &session.threads {
                let current = session.stopped_thread == Some(t.id);
                ui.label(RichText::new(format!("{} {}", if current { "▸" } else { " " }, t.name))
                    .font(prop(12.0))
                    .color(if current { theme.text } else { theme.subtext1 }));
            }
        }

        // ── Call stack ────────────────────────────────────────────────────
        if !session.frames.is_empty() {
            ui.add_space(4.0);
            section_label(ui, theme, "CALL STACK");
            for f in &session.frames {
                let short = f.source.as_deref()
                    .map(|p| p.rsplit('/').next().unwrap_or(p).to_string())
                    .unwrap_or_else(|| "?".to_string());
                let label = RichText::new(format!("{}  {}:{}:{}", f.name, short, f.line, f.column))
                    .font(mono(12.0))
                    .color(theme.text);
                if ui.add(egui::Button::new(label).frame(false)).clicked()
                    && let Some(path) = &f.source {
                    actions.push(ToolAction::Goto { path: path.clone(), line: f.line });
                }
            }
        }

        // ── Variables tree ────────────────────────────────────────────────
        if let Some(scope_ref) = session.scope_ref {
            let roots = session.variable_scopes.get(&scope_ref).cloned();
            if let Some(vars) = roots.filter(|v| !v.is_empty()) {
                ui.add_space(4.0);
                section_label(ui, theme, "VARIABLES");
                render_variable_rows(ui, theme, &session, &vars, 0, &mut actions);
            }
        }

        // ── Watches ───────────────────────────────────────────────────────
        ui.add_space(4.0);
        section_label(ui, theme, "WATCHES");
        for (i, expr) in editor.debug.watches.clone().iter().enumerate() {
            let result = session.watch_results.get(i).cloned().flatten();
            ui.horizontal(|ui| {
                ui.label(RichText::new(expr.clone()).font(mono(12.0)).color(theme.lavender));
                match result {
                    Some(v) => ui.label(RichText::new(format!("= {v}")).font(mono(12.0)).color(theme.text)),
                    None if session.stopped => ui.label(RichText::new("…").font(mono(12.0)).color(theme.overlay0)),
                    None => ui.label(RichText::new("(stopped?)").font(mono(11.0)).color(theme.overlay0)),
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
            if (ui.button("+ Watch").clicked() || (enter && !self.watch_input.trim().is_empty()))
                && !self.watch_input.trim().is_empty() {
                actions.push(ToolAction::WatchAdd(self.watch_input.trim().to_string()));
                self.watch_input.clear();
            }
        });

        // ── Console ───────────────────────────────────────────────────────
        ui.add_space(4.0);
        section_label(ui, theme, "CONSOLE");
        egui::ScrollArea::vertical()
            .max_height(120.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &session.console {
                    ui.label(RichText::new(line.clone()).font(mono(11.0)).color(theme.subtext1));
                }
            });
        ui.horizontal(|ui| {
            let resp = ui.add(egui::TextEdit::singleline(&mut self.eval_input)
                .hint_text("evaluate in stopped context…")
                .font(mono(12.0))
                .desired_width(f32::INFINITY));
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter)) && resp.has_focus();
            if (ui.button("Eval").clicked() || (enter && !self.eval_input.trim().is_empty()))
                && !self.eval_input.trim().is_empty() {
                actions.push(ToolAction::DebugEval(self.eval_input.trim().to_string()));
                self.eval_input.clear();
            }
        });

        actions
    }
}

fn render_variable_rows(
    ui: &mut egui::Ui,
    theme: &GuiTheme,
    session: &crate::kernel::debug::DebugSession,
    vars: &[crate::kernel::debug::Variable],
    depth: usize,
    actions: &mut Vec<ToolAction>,
) {
    for var in vars {
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 14.0);
            let expandable = var.variables_reference > 0;
            let fetched = session.variable_scopes.contains_key(&var.variables_reference);
            let marker = if !expandable { " " }
                else if fetched { "▾" } else { "▸" };
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
