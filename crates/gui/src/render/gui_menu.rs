//! Top menu bar (JetBrains File/Edit/View/Navigate/Run).  Every entry is either
//! a registry command or a shell action — no GUI-only behavior.

use eframe::egui::{self, RichText};

use kernel::state::Editor;

use super::gui_dialog::DialogKind;
use super::gui_layout::LeftTool;
use super::gui_theme::GuiTheme;

/// A menu selection, applied by the shell.
pub enum MenuAction {
    Command(&'static str),
    OpenDialog(DialogKind),
    ShowLeft(LeftTool),
    ToggleTheme,
}

enum Item {
    Cmd(&'static str, &'static str),
    Dialog(&'static str, DialogKind),
    Left(&'static str, LeftTool),
    Theme(&'static str),
    Sep,
}

struct Menu(&'static str, &'static [Item]);

const MENUS: &[Menu] = &[
    Menu("File", &[
        Item::Cmd("Save", "save-buffer"),
        Item::Cmd("Save All", "save-all"),
        Item::Sep,
        Item::Cmd("Close Buffer", "close-buffer"),
        Item::Sep,
        Item::Cmd("Quit", "quit"),
    ]),
    Menu("Edit", &[
        Item::Cmd("Undo", "undo"),
        Item::Cmd("Redo", "redo"),
        Item::Sep,
        Item::Dialog("Find in Buffers…", DialogKind::Search),
    ]),
    Menu("View", &[
        Item::Left("Project", LeftTool::Project),
        Item::Left("Commit", LeftTool::Commit),
        Item::Left("Debugger", LeftTool::Debug),
        Item::Sep,
        Item::Dialog("Go to File…", DialogKind::GotoFile),
        Item::Dialog("Go to Symbol…", DialogKind::GotoSymbol),
        Item::Sep,
        Item::Dialog("Settings…", DialogKind::Settings),
        Item::Theme("Toggle Theme"),
    ]),
    Menu("Navigate", &[
        Item::Cmd("Next Tab", "buffer-next"),
        Item::Cmd("Previous Tab", "buffer-prev"),
        Item::Sep,
        Item::Dialog("Go to File…", DialogKind::GotoFile),
    ]),
    Menu("Run", &[
        Item::Cmd("Run", "run-config"),
        Item::Cmd("Debug", "debug-config"),
        Item::Sep,
        Item::Dialog("Edit Configurations…", DialogKind::RunConfig),
    ]),
];

pub struct MenuBar;

impl MenuBar {
    /// Draw the menu buttons into the caller's horizontal layout; the menu
    /// shares the top row with the main toolbar.
    pub fn show(ui: &mut egui::Ui, _editor: &Editor, theme: &GuiTheme) -> Vec<MenuAction> {
        let mut actions = Vec::new();
        for Menu(label, items) in MENUS {
            ui.menu_button(RichText::new(*label).size(12.0).color(theme.subtext1), |ui| {
                ui.set_min_width(190.0);
                for item in *items {
                    match item {
                        Item::Sep => { ui.separator(); }
                        Item::Cmd(text, cmd) => {
                            if ui.button(*text).clicked() {
                                actions.push(MenuAction::Command(cmd));
                                ui.close();
                            }
                        }
                        Item::Dialog(text, kind) => {
                            if ui.button(*text).clicked() {
                                actions.push(MenuAction::OpenDialog(*kind));
                                ui.close();
                            }
                        }
                        Item::Left(text, tool) => {
                            if ui.button(*text).clicked() {
                                actions.push(MenuAction::ShowLeft(*tool));
                                ui.close();
                            }
                        }
                        Item::Theme(text) => {
                            if ui.button(*text).clicked() {
                                actions.push(MenuAction::ToggleTheme);
                                ui.close();
                            }
                        }
                    }
                }
            });
        }
        actions
    }
}
