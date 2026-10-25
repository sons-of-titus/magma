//! Main toolbar — JetBrains-style run controls.  The run-configuration
//! selector, Run/Debug/Stop, and the Build/Test launchers live at the right
//! edge of the top row, drawn as flat icon buttons.  Actions-as-data: the
//! shell applies them at its write boundary.

use eframe::egui::{self, Align2, Color32, FontFamily, FontId, RichText, Sense, Vec2};
use crate::kernel::state::Editor;
use crate::kernel::task::TaskStatus;
use super::gui_theme::{GuiTheme, icons};

pub enum RunAction {
    /// Launch the active configuration.
    Run,
    /// Launch a specific named configuration (the Build/Test launchers).
    RunNamed(String),
    Debug,
    Stop,
    Select(String),
    Edit,
}

pub struct RunBar;

/// A flat 26×26 toolbar icon button: hover fill, tooltip, and a dimmed glyph
/// when disabled.  Returns true only when enabled *and* clicked.  Shared with
/// the Debugger toolbar, which is icon-only the way JetBrains' is.
pub(crate) fn toolbar_button(
    ui:      &mut egui::Ui,
    glyph:   &str,
    enabled: bool,
    color:   Color32,
    tooltip: &str,
    theme:   &GuiTheme,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    let resp = resp.on_hover_text(tooltip);
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, 3, theme.surface1);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::new(15.0, FontFamily::Proportional),
        if enabled { color } else { theme.overlay0.gamma_multiply(0.55) },
    );
    enabled && resp.clicked()
}

impl RunBar {
    /// Draw the run controls into the caller's right-to-left layout, so they
    /// hold the right edge of the top row.
    pub fn show(ui: &mut egui::Ui, ed: &Editor, theme: &GuiTheme) -> Vec<RunAction> {
        let mut actions = Vec::new();
        let names = ed.project_manager.run_configs.names();
        let active = ed.project_manager.run_configs.active.clone()
            .or_else(|| names.first().cloned());
        let running = active.as_deref()
            .and_then(|n| ed.task_scheduler.task_by_name(n))
            .and_then(|id| ed.task_scheduler.status(id))
            == Some(&TaskStatus::Running);
        let debuggable = active.as_deref()
            .and_then(|n| ed.project_manager.run_configs.named(n))
            .map(|c| c.debuggable)
            .unwrap_or(false);
        let has_named = |name: &str| names.iter().any(|n| n == name);

        // Right-to-left draw order, so this reads left→right as
        // Build · Test · [config ▾] · ▶ Run · 🐞 Debug · ■ Stop · ⚙.
        ui.add_space(6.0); // settings gear at the very right edge
        if toolbar_button(ui, icons::SETTINGS, true, theme.subtext0,
            "Edit run configurations", theme) {
            actions.push(RunAction::Edit);
        }
        if toolbar_button(ui, icons::STOP, running, theme.red,
            if running { "Stop" } else { "Nothing running" }, theme) {
            actions.push(RunAction::Stop);
        }
        if toolbar_button(ui, icons::DEBUG, debuggable, theme.green,
            if debuggable { "Debug" } else { "Configuration is not debuggable" }, theme) {
            actions.push(RunAction::Debug);
        }
        let run_tip = match &active {
            Some(n) => format!("Run '{n}'"),
            None => "No run configuration".to_string(),
        };
        if toolbar_button(ui, icons::RUN, active.is_some(), theme.green, &run_tip, theme) {
            actions.push(RunAction::Run);
        }

        let mut selected = active.clone();
        egui::ComboBox::from_id_salt("magma_run_config_select")
            .selected_text(active.as_deref().unwrap_or("No configuration"))
            .width(150.0)
            .show_ui(ui, |ui| {
                for name in &names {
                    ui.selectable_value(&mut selected, Some(name.clone()), name);
                }
                if names.is_empty() {
                    ui.label(RichText::new("no configurations").size(12.0).color(theme.overlay0));
                }
            });
        if selected != active && let Some(name) = selected {
            actions.push(RunAction::Select(name));
        }

        if toolbar_button(ui, icons::TEST, has_named("test"), theme.subtext0, "Test", theme) {
            actions.push(RunAction::RunNamed("test".into()));
        }
        if toolbar_button(ui, icons::BUILD, has_named("build"), theme.subtext0, "Build", theme) {
            actions.push(RunAction::RunNamed("build".into()));
        }

        actions
    }
}
