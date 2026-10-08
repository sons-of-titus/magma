//! Welcome window — the first minute of the product.
//!
//! The welcome screen is its own window (the app's primary window): a
//! folder picker entry point, the recent-projects list, and the minimum key
//! vocabulary.  Opening a project opens the main editor window and hides
//! this one.  Every action maps to a registry command (`open-project`), so
//! Janet-side behavior stays the single authority.

use eframe::egui::{self, Align2, FontFamily, FontId, Pos2, RichText, Sense, Vec2};
use crate::kernel::state::Editor;
use super::gui_theme::GuiTheme;

/// What the user asked the shell to do from the welcome window.
pub enum WelcomeAction {
    OpenFolder,
    OpenProject(String),
}

const WORDMARK_SIZE: f32 = 34.0;

#[derive(Default)]
pub struct WelcomeScreen;

impl WelcomeScreen {
    pub fn show(
        &self,
        ui: &mut egui::Ui,
        editor: &Editor,
        theme: &GuiTheme,
    ) -> Option<WelcomeAction> {
        let mut action = None;
        let avail = ui.available_size();

        let sans = |size: f32| FontId::new(size, FontFamily::Proportional);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(avail.y * 0.12);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Magma")
                        .font(sans(WORDMARK_SIZE))
                        .color(theme.text)
                        .strong(),
                );
                ui.label(
                    RichText::new(concat!("A programmable editor — Rust kernel, Janet userland  ·  v", env!("CARGO_PKG_VERSION")))
                        .font(sans(12.0))
                        .color(theme.overlay0),
                );
                ui.add_space(20.0);

                let open = ui.add_sized(
                    [220.0, 34.0],
                    egui::Button::new(
                        RichText::new("Open Folder").font(sans(14.0)).color(theme.base),
                    ).fill(theme.blue),
                );
                if open.clicked() {
                    action = Some(WelcomeAction::OpenFolder);
                }

                ui.add_space(28.0);
            });

            // ── Recent projects ───────────────────────────────────────
            let recents = &editor.project_manager.recent_projects;
            if !recents.is_empty() {
                let x0 = (ui.available_width() - 380.0).max(24.0) / 2.0 + ui.cursor().left();
                ui.label(
                    RichText::new("RECENT PROJECTS")
                        .font(sans(11.0))
                        .color(theme.overlay0)
                        .strong(),
                );
                ui.add_space(4.0);
                for path in recents.iter().take(8) {
                    let (_, resp) = ui.allocate_exact_size(
                        Vec2::new(380.0, 24.0),
                        Sense::click(),
                    );
                    let hovered = resp.hovered();
                    if hovered {
                        ui.painter().rect_filled(resp.rect, egui::CornerRadius::same(4), theme.surface0);
                    }
                    ui.painter().text(
                        Pos2::new(x0 + 8.0, resp.rect.center().y),
                        Align2::LEFT_CENTER,
                        path,
                        sans(13.0),
                        if hovered { theme.text } else { theme.subtext1 },
                    );
                    if resp.clicked() {
                        action = Some(WelcomeAction::OpenProject(path.clone()));
                    }
                }
                ui.add_space(24.0);
            }

            ui.vertical_centered(|ui| {
                ui.separator();
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "ctrl-p search everywhere      ctrl-shift-n go to file      ctrl-space complete      ctrl-\\ sidebar",
                    )
                    .font(sans(12.0))
                    .color(theme.overlay0),
                );
            });
        });
        action
    }
}
