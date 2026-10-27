//! Command-line dialog — run any command by name with inline arguments
//! (`alt-x`, M-x style).  One input; Enter parses `"name args…"` through
//! `execute_command_line`, Escape closes (shell key gate).

use eframe::egui::{self, FontFamily, FontId, RichText};
use magma::kernel::state::Editor;
use super::gui_dialog::DialogAction;
use super::gui_theme::GuiTheme;

pub struct CommandLineDialog {
    pub input: String,
    focus_pending: bool,
}

impl CommandLineDialog {
    pub fn new() -> Self {
        Self { input: String::new(), focus_pending: false }
    }

    pub fn reset(&mut self) {
        self.input.clear();
        self.focus_pending = true;
    }

    /// Returns true when the dialog should close.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        editor: &Editor,
        theme: &GuiTheme,
        actions: &mut Vec<DialogAction>,
    ) -> bool {
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.input)
                .hint_text("Command and arguments — e.g. open-file src/main.rs")
                .font(FontId::new(13.0, FontFamily::Monospace))
                .desired_width(f32::INFINITY),
        );
        if self.focus_pending {
            resp.request_focus();
            self.focus_pending = false;
        }

        let line = self.input.trim().to_string();
        let name = line.split_whitespace().next().unwrap_or("");
        let known = name.is_empty() || editor.commands.exists(name);
        if !known {
            ui.label(RichText::new(format!("Unknown command: {name}"))
                .size(12.0).color(theme.red));
        }
        ui.label(RichText::new("Enter runs · Esc closes")
            .size(11.0).color(theme.overlay0));

        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if enter && !line.is_empty() && known {
            actions.push(DialogAction::ExecuteCommandLine(line));
            return true;
        }
        false
    }
}
