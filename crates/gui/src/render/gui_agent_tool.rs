//! Agent tool window — the chat surface for the built-in agent: prompt
//! input, the live transcript (thoughts, tool calls, results), and stop
//! control.  Reads session state from `AgentManager`; mutations go through
//! `ToolAction`s applied at the shell boundary.

use eframe::egui::{self, FontFamily, FontId, RichText, ScrollArea};
use kernel::state::Editor;
use super::gui_theme::{GuiTheme, icons};
use super::tool_window::{ToolAction, ToolWindow};

fn mono(size: f32) -> FontId { FontId::new(size, FontFamily::Monospace) }
fn prop(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

pub struct AgentToolWindow {
    input: String,
}

impl AgentToolWindow {
    pub fn new() -> Self {
        Self { input: String::new() }
    }
}

impl ToolWindow for AgentToolWindow {
    fn title(&self) -> &str { "Agent" }
    fn icon(&self)  -> char { super::gui_theme::icons::AGENT.chars().next().unwrap_or(' ') }

    fn ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        let mut actions = Vec::new();

        // Latest session is the active conversation.
        let active = editor.agent.list().pop().cloned();

        let configured = kernel::agent::llm::LlmConfig::is_configured(&editor.options);
        ui.horizontal(|ui| {
            let status = match &active {
                None => "no session — ask below".to_string(),
                Some(s) => format!("session {} · {}", s.id, s.status.as_str()),
            };
            let color = match active.as_ref().map(|s| &s.status) {
                Some(kernel::agent::AgentStatus::Running) => theme.blue,
                Some(kernel::agent::AgentStatus::Failed) => theme.red,
                Some(kernel::agent::AgentStatus::Completed) => theme.green,
                None => theme.overlay0,
            };
            ui.label(RichText::new(status).font(prop(12.0)).color(color).strong());
            if !configured {
                ui.label(RichText::new("· no API key — deterministic planner")
                    .font(prop(11.0)).color(theme.overlay0));
            }
        });
        ui.separator();

        // ── Transcript ────────────────────────────────────────────────────
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if let Some(session) = &active {
                    ui.label(RichText::new(format!("You: {}", session.prompt))
                        .font(prop(12.5)).color(theme.text).strong());
                    ui.add_space(4.0);
                    for entry in &session.transcript {
                        match entry {
                            kernel::agent::TranscriptEntry::Thought(text) => {
                                ui.label(RichText::new(text)
                                    .font(prop(12.0)).color(theme.text));
                            }
                            kernel::agent::TranscriptEntry::Action { command, description } => {
                                let label = if command.is_empty() {
                                    description.clone()
                                } else {
                                    format!("{command} — {description}")
                                };
                                ui.label(RichText::new(format!("{} {label}", icons::CARET_RIGHT))
                                    .font(mono(11.5)).color(theme.blue));
                            }
                            kernel::agent::TranscriptEntry::StepOutput(text) => {
                                ui.label(RichText::new(text)
                                    .font(mono(11.0)).color(theme.subtext1));
                            }
                        }
                        ui.add_space(2.0);
                    }
                    if let Some(err) = &session.error {
                        ui.label(RichText::new(format!("error: {err}"))
                            .font(prop(12.0)).color(theme.red));
                    }
                    if let Some(result) = &session.result {
                        ui.add_space(4.0);
                        ui.separator();
                        ui.label(RichText::new(result)
                            .font(prop(12.5)).color(theme.green).strong());
                    }
                }
            });

        ui.separator();

        // ── Input + controls ─────────────────────────────────────────────
        let running = active.as_ref()
            .map(|s| s.status == kernel::agent::AgentStatus::Running)
            .unwrap_or(false);
        ui.horizontal(|ui| {
            // A bounded width — an infinite desired width would let the
            // docked panel grow to fill the window.
            let input_w = (ui.available_width() - 120.0).max(80.0);
            let resp = ui.add(egui::TextEdit::singleline(&mut self.input)
                .hint_text("ask the agent…")
                .font(prop(12.5))
                .desired_width(input_w));
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter)) && resp.has_focus();
            let prompt = self.input.trim().to_string();
            if (ui.button("Send").clicked() || (enter && !prompt.is_empty()))
                && !prompt.is_empty() {
                actions.push(ToolAction::AgentSubmit(prompt));
                self.input.clear();
            }
            if running && ui.button("Stop").clicked()
                && let Some(s) = &active {
                    actions.push(ToolAction::AgentStop(s.id));
                }
        });

        actions
    }
}
