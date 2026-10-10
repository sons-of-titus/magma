//! The three welcome content panes: Projects (the home surface), Customize,
//! and Learn.  Pure layout — every mutation goes back as a [`WelcomeAction`].

use eframe::egui::{self, Align, Align2, Color32, FontId, Pos2, RichText, Sense, Stroke, Vec2};

use crate::kernel::state::Editor;
use super::gui_theme::{GuiTheme, icons};
use super::gui_welcome::{WelcomeAction, outline_button, sans, section_header};

const RECENTS_SHOWN: usize = 8;

/// Projects — wordmark, the New Project / Open entry points, and the recent
/// list.  Up/Down move the keyboard selection, Enter opens it, hover reveals
/// the forget control.
pub(crate) fn projects(
    ui: &mut egui::Ui,
    editor: &Editor,
    theme: &GuiTheme,
    selected: &mut Option<usize>,
) -> Option<WelcomeAction> {
    let mut action = None;

    ui.label(RichText::new("Magma").font(sans(34.0)).color(theme.text).strong());
    ui.label(
        RichText::new(concat!(
            "A programmable editor — Rust kernel, Janet userland  ·  v",
            env!("CARGO_PKG_VERSION")))
        .font(sans(12.0))
        .color(theme.overlay0),
    );
    ui.add_space(28.0);

    ui.horizontal(|ui| {
        if outline_button(ui, theme, "New Project", true) {
            action = Some(WelcomeAction::NewProject);
        }
        if outline_button(ui, theme, "Open", false) {
            action = Some(WelcomeAction::OpenFolder);
        }
    });
    ui.add_space(26.0);

    let recents = &editor.project_manager.recent_projects;
    let n = recents.len().min(RECENTS_SHOWN);
    if n == 0 {
        *selected = None;
        ui.label(RichText::new("No recent projects — open a folder to get started.")
            .font(sans(13.0))
            .color(theme.overlay0));
        return action;
    }

    let (up, down, enter) = ui.ctx().input(|i| (
        i.key_pressed(egui::Key::ArrowUp),
        i.key_pressed(egui::Key::ArrowDown),
        i.key_pressed(egui::Key::Enter),
    ));
    if up || down {
        *selected = Some(match (*selected, down) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(i), true) => (i + 1).min(n - 1),
            (Some(i), false) => i.saturating_sub(1),
        });
    }
    if let Some(i) = *selected
        && i >= n {
        *selected = None; // the list shrank under the selection
    }
    if enter
        && let Some(i) = *selected {
        action = Some(WelcomeAction::OpenProject(recents[i].clone()));
    }

    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        for (i, path) in recents.iter().enumerate().take(RECENTS_SHOWN) {
            let is_selected = *selected == Some(i);
            let row_w = ui.available_width().min(560.0);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(row_w, 44.0), Sense::click());
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);

            let hovered = resp.hovered();
            if is_selected || hovered {
                let fill = if is_selected {
                    theme.surface0
                } else {
                    theme.surface0.gamma_multiply(0.55)
                };
                ui.painter().rect_filled(rect, 6, fill);
            }
            let clip = ui.painter_at(rect);
            clip.text(
                Pos2::new(rect.left() + 12.0, rect.top() + 16.0),
                Align2::LEFT_CENTER,
                display_name(path),
                sans(14.0),
                theme.text,
            );
            let path_font = sans(11.5);
            let max_w = row_w - if hovered { 52.0 } else { 24.0 };
            clip.text(
                Pos2::new(rect.left() + 12.0, rect.top() + 31.0),
                Align2::LEFT_CENTER,
                &fit_text(ui, path, &path_font, max_w),
                path_font,
                theme.overlay0,
            );
            if is_selected {
                ui.scroll_to_rect(rect, Some(Align::Center));
            }

            let mut removed = false;
            if hovered {
                let icon_rect = egui::Rect::from_center_size(
                    Pos2::new(rect.right() - 22.0, rect.center().y),
                    Vec2::splat(22.0),
                );
                let icon = ui.interact(icon_rect, egui::Id::new(("welcome-forget", path)), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("Remove from recent projects");
                let color = if icon.hovered() { theme.red } else { theme.overlay0 };
                clip.text(icon_rect.center(), Align2::CENTER_CENTER, icons::CLOSE, sans(14.0), color);
                if icon.clicked() {
                    action = Some(WelcomeAction::RemoveRecent(path.clone()));
                    *selected = None;
                    removed = true;
                }
            }
            if !removed && resp.clicked() {
                action = Some(WelcomeAction::OpenProject(path.clone()));
            }
            ui.add_space(4.0);
        }
    });
    action
}

/// Customize — color theme and editor font size, with a live preview of the
/// size so the controls have immediate feedback.
pub(crate) fn customize(ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) -> Option<WelcomeAction> {
    let mut action = None;

    ui.label(RichText::new("Customize").font(sans(22.0)).color(theme.text).strong());
    ui.add_space(18.0);

    section_header(ui, theme, "COLOR THEME");
    let light = GuiTheme::option_is_light(&editor.options);
    for (key, label, hint) in [
        ("mocha", "Catppuccin Mocha", "dark"),
        ("latte", "Catppuccin Latte", "light"),
    ] {
        if option_row(ui, theme, label, hint, (key == "latte") == light) {
            action = Some(WelcomeAction::SetTheme(key));
        }
    }

    ui.add_space(16.0);
    section_header(ui, theme, "EDITOR FONT SIZE");
    ui.horizontal(|ui| {
        if mini_button(ui, theme, "-") {
            action = Some(WelcomeAction::FontSize(-1.0));
        }
        ui.add_space(6.0);
        ui.label(RichText::new(format!("{} pt", editor.font_config.size)).font(sans(14.0)).color(theme.text));
        ui.add_space(6.0);
        if mini_button(ui, theme, "+") {
            action = Some(WelcomeAction::FontSize(1.0));
        }
    });
    ui.add_space(12.0);
    ui.label(
        RichText::new("fn main() { forge(\"magma\"); }  // live preview")
            .font(FontId::monospace(editor.font_config.size))
            .color(theme.subtext1),
    );
    action
}

/// Learn — the key vocabulary a newcomer needs first.
pub(crate) fn learn(ui: &mut egui::Ui, theme: &GuiTheme) -> Option<WelcomeAction> {
    ui.label(RichText::new("Learn").font(sans(22.0)).color(theme.text).strong());
    ui.add_space(4.0);
    ui.label(RichText::new("The chords that matter, in the order you'll need them.")
        .font(sans(12.0))
        .color(theme.overlay0));
    ui.add_space(16.0);

    const KEYS: &[(&str, &str)] = &[
        ("ctrl-p",       "Search everywhere — commands, files, symbols"),
        ("ctrl-shift-n", "Go to file"),
        ("ctrl-space",   "Trigger completion"),
        ("ctrl-\\",      "Toggle the sidebar"),
        ("ctrl-s",       "Save the buffer"),
        ("ctrl-shift-l", "Toggle light / dark theme"),
        ("ctrl-q",       "Quit"),
    ];
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        for &(chord, description) in KEYS {
            ui.horizontal(|ui| {
                let font = FontId::monospace(11.5);
                let w = ui.ctx()
                    .fonts_mut(|f| f.layout_no_wrap(chord.to_string(), font.clone(), theme.subtext1))
                    .size().x;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(w.max(96.0) + 16.0, 22.0), Sense::hover());
                ui.painter().rect_filled(rect, 4, theme.surface0);
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, chord, font, theme.subtext1);
                ui.add_space(10.0);
                ui.label(RichText::new(description).font(sans(13.0)).color(theme.subtext1));
            });
            ui.add_space(4.0);
        }
        ui.add_space(14.0);
        ui.label(RichText::new("The full keymap lives in Janet — see builtins/editor_keybindings.janet.")
            .font(sans(11.0))
            .color(theme.overlay0));
    });
    None
}

fn option_row(ui: &mut egui::Ui, theme: &GuiTheme, label: &str, hint: &str, active: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(300.0, 30.0), Sense::click());
    if active {
        ui.painter().rect_filled(rect, 6, theme.surface0);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 6, theme.surface0.gamma_multiply(0.5));
    }
    if active {
        ui.painter().circle_filled(Pos2::new(rect.left() + 16.0, rect.center().y), 3.5, theme.blue);
    }
    ui.painter().text(
        Pos2::new(rect.left() + 32.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        sans(13.5),
        if active { theme.text } else { theme.subtext1 },
    );
    ui.painter().text(
        Pos2::new(rect.right() - 14.0, rect.center().y),
        Align2::RIGHT_CENTER,
        hint,
        sans(11.0),
        theme.overlay0,
    );
    resp.clicked()
}

fn mini_button(ui: &mut egui::Ui, theme: &GuiTheme, glyph: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 6, theme.surface0.gamma_multiply(0.5));
    }
    let stroke = if resp.hovered() {
        Stroke::new(1.0, theme.surface1)
    } else {
        Stroke::new(1.0, theme.surface0)
    };
    ui.painter().rect_stroke(rect, 6.0, stroke, egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, glyph, sans(14.0), theme.subtext1);
    resp.clicked()
}

/// A recent project's display name: its last path segment.
fn display_name(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    trimmed.rsplit('/').next().unwrap_or(trimmed)
}

/// Truncate with an ellipsis so a path never outgrows its row.
fn fit_text(ui: &egui::Ui, text: &str, font: &FontId, max_w: f32) -> String {
    let width = |s: &str| ui.ctx()
        .fonts_mut(|f| f.layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE))
        .size().x;
    if width(text) <= max_w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 1;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let mut cand: String = chars[..mid].iter().collect();
        cand.push('…');
        if width(&cand) <= max_w {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let mut out: String = chars[..lo.saturating_sub(1)].iter().collect();
    out.push('…');
    out
}
