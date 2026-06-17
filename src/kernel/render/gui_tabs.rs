//! Native tab bar widget for the GUI.

use eframe::egui;
use crate::kernel::state::id::WindowId;
use crate::kernel::state::Editor;

pub enum TabAction {
    Focus(WindowId),
    Close(WindowId),
}

impl TabAction {
    pub fn apply(self, editor: &mut Editor) {
        match self {
            TabAction::Focus(id) => editor.view_tree.focus(id),
            TabAction::Close(id) => editor.view_tree.close(id),
        }
    }
}

pub struct TabBar;

impl TabBar {
    /// Render the tab bar and return any user action that should be applied.
    pub fn show(ui: &mut egui::Ui, editor: &Editor) -> Option<TabAction> {
        let mut action: Option<TabAction> = None;
        let focused = editor.view_tree.focused_window();

        ui.horizontal(|ui| {
            ui.set_min_height(24.0);
            for pane in editor.view_tree.panes() {
                let Some(buf_id) = pane.buffer_id else { continue };
                let is_active = focused == Some(pane.id);

                let (name, modified) = editor.buffers.get(buf_id)
                    .map(|arc| {
                        let buf = arc.lock().unwrap();
                        (buf.name.clone(), buf.modified())
                    })
                    .unwrap_or_else(|| ("?".to_string(), false));

                let display = name.rsplit('/').next().unwrap_or(&name).to_string();
                let label = if modified {
                    format!("● {}", display)
                } else {
                    display
                };

                let pane_id = pane.id;
                let resp = ui.selectable_label(is_active, &label);
                if resp.clicked() && !is_active && action.is_none() {
                    action = Some(TabAction::Focus(pane_id));
                }

                let close = ui.small_button("✕");
                if close.clicked() && action.is_none() {
                    action = Some(TabAction::Close(pane_id));
                }

                ui.add_space(4.0);
            }
        });

        action
    }
}
