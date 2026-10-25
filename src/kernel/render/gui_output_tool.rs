//! Output tool window — build/task output with clickable problems.
//!
//! `:build` / `:test` / `:run` land their output here (`editor.last_task`)
//! and their parsed errors in the shared quickfix list, so the edit → build
//! → jump loop finally closes.

use eframe::egui::{self, FontFamily, FontId, RichText};
use crate::kernel::state::Editor;
use super::gui_find_tool::quickfix_rows;
use super::gui_theme::{GuiTheme, icons};
use super::tool_window::{ToolAction, ToolWindow};

fn mono(size: f32) -> FontId { FontId::new(size, FontFamily::Monospace) }
fn prop(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

pub struct OutputToolWindow;

impl OutputToolWindow {
    pub fn new() -> Self { Self }
}

impl ToolWindow for OutputToolWindow {
    fn title(&self) -> &str { "Output" }
    fn icon(&self)  -> char { super::gui_theme::icons::OUTPUT.chars().next().unwrap_or(' ') }

    fn ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        match &editor.last_task {
            Some((name, output)) => {
                ui.label(
                    RichText::new(format!("{} {name}", icons::CARET_RIGHT))
                        .font(prop(12.0))
                        .color(theme.blue)
                        .strong(),
                );
                egui::ScrollArea::vertical()
                    .id_salt("magma_output_text")
                    .auto_shrink([false, false])
                    .max_height(140.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(output).font(mono(12.0)).color(theme.text),
                            )
                            .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    });
            }
            None => {
                ui.label(
                    RichText::new("No task has run yet.  Try :build, :test, or :run.")
                        .font(prop(12.0))
                        .color(theme.overlay0),
                );
            }
        }

        ui.separator();
        let problems = editor.quickfix_list.len();
        ui.label(
            RichText::new(if problems > 0 {
                format!("PROBLEMS ({problems})")
            } else {
                "PROBLEMS".to_string()
            })
            .font(prop(11.0))
            .color(theme.overlay0)
            .strong(),
        );
        quickfix_rows(ui, editor, theme)
    }
}
