//! Welcome window — the first minute of the product.
//!
//! A JetBrains-style greeter: a navigation rail (Projects, Customize, Learn)
//! beside a content pane.  Projects is the home surface — wordmark, the New
//! Project / Open entry points, and the recent-projects list with
//! hover-to-forget; Customize carries theme and editor-font controls; Learn
//! the key vocabulary.  Opening a project opens the main editor window and
//! hides this one.  Actions flow back through one write-lock boundary
//! (`ShellRuntime::handle_welcome_action`), so Janet-side behavior stays the
//! single authority.

use eframe::egui::{self, Align2, FontFamily, FontId, Pos2, RichText, Sense, Stroke, Vec2};

use crate::kernel::state::Editor;
use super::gui_theme::{GuiTheme, icons};
use super::gui_welcome_panes;

/// What the user asked the shell to do from the welcome window.
pub enum WelcomeAction {
    OpenFolder,
    NewProject,
    OpenProject(String),
    RemoveRecent(String),
    /// Switch the `gui-theme` preset (`"mocha"` | `"latte"`).
    SetTheme(&'static str),
    /// Step the editor font size, in points.
    FontSize(f32),
}

#[derive(Default, Clone, Copy, PartialEq)]
enum Pane {
    #[default]
    Projects,
    Customize,
    Learn,
}

pub struct WelcomeScreen {
    pane: Pane,
    /// Keyboard-focused row of the recent list (Up/Down/Enter).
    selected_recent: Option<usize>,
}

impl Default for WelcomeScreen {
    fn default() -> Self {
        Self { pane: Pane::Projects, selected_recent: None }
    }
}

const SIDEBAR_W: f32 = 190.0;

impl WelcomeScreen {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        editor: &Editor,
        theme: &GuiTheme,
    ) -> Option<WelcomeAction> {
        let mut action = None;

        egui::containers::Panel::left("welcome_nav")
            .resizable(false)
            .exact_size(SIDEBAR_W)
            .frame(egui::Frame::NONE.fill(theme.base)
                .inner_margin(egui::Margin { left: 12, right: 10, top: 16, bottom: 12 }))
            .show(ui, |ui| self.sidebar(ui, theme));

        egui::containers::CentralPanel::no_frame()
            .frame(egui::Frame::NONE.fill(theme.base)
                .inner_margin(egui::Margin { left: 44, right: 44, top: 34, bottom: 20 }))
            .show(ui, |ui| {
                action = match self.pane {
                    Pane::Projects => gui_welcome_panes::projects(
                        ui, editor, theme, &mut self.selected_recent),
                    Pane::Customize => gui_welcome_panes::customize(ui, editor, theme),
                    Pane::Learn => gui_welcome_panes::learn(ui, theme),
                };
            });
        action
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, theme: &GuiTheme) {
        for (pane, icon, label) in [
            (Pane::Projects,  icons::PROJECTS, "Projects"),
            (Pane::Customize, icons::SETTINGS,  "Customize"),
            (Pane::Learn,     icons::LEARN,    "Learn"),
        ] {
            if self.nav_item(ui, theme, icon, label, self.pane == pane) {
                self.pane = pane;
                self.selected_recent = None;
            }
        }
        // Bottom utilities, JetBrains options/help placement.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if self.icon_button(ui, theme, icons::SETTINGS, "Customize") {
                    self.pane = Pane::Customize;
                }
                if self.icon_button(ui, theme, "?", "Key vocabulary") {
                    self.pane = Pane::Learn;
                }
            });
        });
    }

    fn nav_item(
        &self,
        ui: &mut egui::Ui,
        theme: &GuiTheme,
        icon: &str,
        label: &str,
        selected: bool,
    ) -> bool {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
        if selected {
            ui.painter().rect_filled(rect, 6, theme.surface0);
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 6, theme.surface0.gamma_multiply(0.5));
        }
        ui.painter().text(
            Pos2::new(rect.left() + 12.0, rect.center().y),
            Align2::LEFT_CENTER,
            icon,
            sans(13.0),
            if selected { theme.blue } else { theme.overlay0 },
        );
        ui.painter().text(
            Pos2::new(rect.left() + 36.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            sans(14.0),
            if selected { theme.text } else { theme.subtext1 },
        );
        resp.clicked()
    }

    fn icon_button(&self, ui: &mut egui::Ui, theme: &GuiTheme, glyph: &str, tip: &str) -> bool {
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, 6, theme.surface0.gamma_multiply(0.5));
        }
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, glyph, sans(14.0), theme.overlay0);
        resp.on_hover_text(tip).clicked()
    }
}

/// egui chrome text: the default Proportional family at a fixed size.
pub(crate) fn sans(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Small caps group header used above the Customize/Learn sections.
pub(crate) fn section_header(ui: &mut egui::Ui, theme: &GuiTheme, label: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(label).font(sans(11.0)).color(theme.overlay0).strong());
    ui.add_space(4.0);
}

/// Flat outlined button, JetBrains welcome style: accent variant for the
/// primary action, neutral for the rest.
pub(crate) fn outline_button(ui: &mut egui::Ui, theme: &GuiTheme, label: &str, accent: bool) -> bool {
    let font = sans(13.5);
    let text_w = ui.painter()
        .layout_no_wrap(label.to_owned(), font.clone(), theme.text)
        .size().x;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new((text_w + 32.0).max(112.0), 30.0), Sense::click());
    let (stroke, color, hover_fill) = if accent {
        (Stroke::new(1.2, theme.blue), theme.blue, theme.blue.gamma_multiply(0.13))
    } else {
        (Stroke::new(1.0, theme.surface1), theme.subtext1, theme.surface0.gamma_multiply(0.5))
    };
    if resp.hovered() || resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, 6, hover_fill);
    }
    ui.painter().rect_stroke(rect, 6.0, stroke, egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, color);
    resp.clicked()
}
