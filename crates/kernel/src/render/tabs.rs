//! Tab projection — the editor-state view of what a tab strip shows.
//!
//! The frontend decides how a tab looks; this module decides which panes are
//! tabs, which label each carries, and that a side-by-side diff reads as one
//! tab.  Keeping it in the kernel lets headless code and kernel tests describe
//! tabs without naming a widget toolkit.

use crate::icons::IconKind;
use crate::state::Editor;
use crate::state::id::WindowId;

#[derive(Debug, Clone)]
pub struct TabModel {
    pub label: String,
    pub window_id: WindowId,
    pub modified: bool,
    pub closable: bool,
    pub icon: Option<IconKind>,
}

pub fn collect_tabs(editor: &Editor) -> Vec<TabModel> {
    let diff = editor.vc.diff_view.as_ref();
    editor
        .view_tree
        .panes()
        .iter()
        .filter_map(|pane| {
            let buf_id = pane.buffer_id?;
            if let Some(view) = diff
                && pane.id == view.right
            {
                return None;
            }
            if let Some(view) = diff
                && pane.id == view.left
            {
                let short = view.path.rsplit('/').next().unwrap_or(&view.path);
                return Some(TabModel {
                    label: format!("{short}:HEAD->TREE"),
                    window_id: pane.id,
                    modified: false,
                    closable: true,
                    icon: None,
                });
            }
            let (label, modified) = editor
                .buffers
                .get(buf_id)
                .map(|arc| {
                    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                    (buf.name.clone(), buf.modified())
                })
                .unwrap_or_else(|| ("?".to_string(), false));
            let short = label.rsplit('/').next().unwrap_or(&label).to_string();
            let icon = (!short.starts_with('*'))
                .then(|| editor.icons.lookup(&short))
                .flatten()
                .cloned();
            Some(TabModel {
                label: short,
                window_id: pane.id,
                modified,
                closable: true,
                icon,
            })
        })
        .collect()
}

/// Close a tab.  A pane belonging to the side-by-side diff closes the whole
/// pair; closing the last tab leaves an empty editor area rather than
/// resurrecting a scratch buffer.  The session flushes here so a closed tab
/// stays closed even if the app never exits cleanly.
pub fn close_tab(editor: &mut Editor, id: WindowId) {
    if let Some(view) = &editor.vc.diff_view
        && (view.left == id || view.right == id)
    {
        crate::command::builtin::vc_diff::close_diff(editor);
        return;
    }
    let was_last = editor.view_tree.panes().len() <= 1;
    editor.view_tree.close(id);
    if was_last && let Some(win) = editor.view_tree.focused_window_mut() {
        win.buffer_id = None;
    }
    crate::session::save_if_project(editor);
}
