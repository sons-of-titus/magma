//! Debug tool window — the real DAP surface.
//!
//! Renders the active session's stopped state, threads, stack frames
//! (click → open at the frame's line), and variables.  The data arrives
//! through the kernel's DAP response chain (runtime.rs populates the
//! session on stackTrace/scopes/variables).

use eframe::egui::{self, FontFamily, FontId, RichText};
use crate::kernel::state::Editor;
use super::gui_theme::GuiTheme;
use super::tool_window::{ToolAction, ToolWindow};

fn mono(size: f32) -> FontId { FontId::new(size, FontFamily::Monospace) }
fn prop(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

pub struct DebugToolWindow;

impl DebugToolWindow {
    pub fn new() -> Self { Self }
}

impl ToolWindow for DebugToolWindow {
    fn title(&self) -> &str { "Debug" }
    fn icon(&self)  -> char { '֍' }

    fn ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        let Some(session) = editor.debug.active_session() else {
            ui.label(
                RichText::new("No debug session.  Start one with :dbg-start <adapter>.")
                    .font(prop(12.0))
                    .color(theme.overlay0),
            );
            return Vec::new();
        };

        let state = if session.stopped { "STOPPED" } else { "RUNNING" };
        let state_color = if session.stopped { theme.red } else { theme.green };
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}  ·  {}", session.adapter, state))
                .font(prop(12.0)).color(state_color).strong());
        });
        ui.separator();

        let mut actions = Vec::new();

        if !session.threads.is_empty() {
            ui.label(RichText::new("THREADS").font(prop(10.0)).color(theme.overlay0).strong());
            for t in &session.threads {
                let current = session.stopped_thread == Some(t.id);
                ui.label(RichText::new(format!("{} {}", if current { "▸" } else { " " }, t.name))
                    .font(prop(12.0))
                    .color(if current { theme.text } else { theme.subtext1 }));
            }
        }

        if !session.frames.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("CALL STACK").font(prop(10.0)).color(theme.overlay0).strong());
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

        if !session.variables.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("VARIABLES").font(prop(10.0)).color(theme.overlay0).strong());
            for var in &session.variables {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(var.name.clone()).font(mono(12.0)).color(theme.lavender));
                    ui.label(RichText::new(var.value.clone()).font(mono(12.0)).color(theme.text));
                });
            }
        }

        if session.frames.is_empty() && session.variables.is_empty() {
            ui.label(
                RichText::new("Stopped with no stack loaded yet — frames appear when the adapter reports one.")
                    .font(prop(11.0))
                    .color(theme.overlay0),
            );
        }
        actions
    }
}
