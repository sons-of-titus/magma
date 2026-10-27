/// An overlay (floating window) drawn above buffer content.
#[derive(Debug, Clone)]
pub struct Overlay {
    pub id: usize,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    /// Buffer whose content fills the overlay area, if any.
    pub buffer_id: Option<usize>,
    /// Higher z_order is drawn on top.
    pub z_order: i32,
}

/// Snapshot of the last-rendered frame's layout, written by `render_frame`
/// each frame so `input::mouse::dispatch_mouse` can resolve click coordinates
/// without recomputing the gutter / content layout from scratch.
#[derive(Debug, Clone, Default)]
pub struct LayoutSnapshot {
    /// Total gutter width in cells (sum of all visible column widths).
    pub gutter_width: usize,
    /// X column where buffer content begins (gutter_width + prefix_margin).
    pub content_x: u16,
    /// Number of rows consumed by the tab bar + header line at the top.
    pub row_offset: u16,
    /// Scroll offset (first visible buffer line index) at the time of rendering.
    pub scroll_top: usize,
    /// Number of content rows rendered (excluding status bar).
    pub visible_lines: usize,
}

/// Splice server hover text into the visible hover card (server text
/// first), or surface it as a status message when no card is showing —
/// that is the manual `lsp-hover` path.  Called from the event pump.
pub fn attach_lsp_hover(ed: &mut crate::state::Editor, buf_key: usize, contents: &str) {
    let focused = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid));
    if focused == Some(buf_key)
        && let Some(card) = ed.hover.as_mut()
    {
        let mut lines: Vec<String> = contents.lines()
            .map(str::trim_end)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        lines.append(&mut card.lines);
        card.lines = lines;
        return;
    }
    let first = contents.lines().find(|l| !l.trim().is_empty()).unwrap_or(contents);
    ed.show_message(crate::state::MessageKind::Info,
        first.chars().take(160).collect::<String>());
}

/// One code action from the language server.
#[derive(Debug, Clone)]
pub struct CodeAction {
    pub title: String,
    pub kind: String,
    /// The action's `edit` (WorkspaceEdit JSON) when the server sent it
    /// inline; actions that only carry a `command` apply nothing yet.
    pub edit_json: Option<String>,
}

/// The quick-fix picker: server code actions for one buffer, navigated
/// with up/down, applied with return, dismissed with escape.
#[derive(Debug, Clone)]
pub struct QuickfixMenu {
    pub buf_key: usize,
    pub items: Vec<CodeAction>,
    pub idx: usize,
}

/// Parse a `textDocument/codeAction` result into picker items.
pub fn parse_code_actions(json: &str) -> Vec<CodeAction> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(arr) = v.as_array() else { return Vec::new() };
    arr.iter().filter_map(|a| {
        let title = a.get("title")?.as_str()?.to_string();
        let kind = a.get("kind").and_then(|k| k.as_str()).unwrap_or_default().to_string();
        let edit_json = a.get("edit").map(|e| e.to_string());
        Some(CodeAction { title, kind, edit_json })
    }).collect()
}

/// Quickfix-menu operations shared by the input layer and tests.
pub fn quickfix_menu_next(ed: &mut crate::state::Editor) {
    if let Some(menu) = ed.quickfix_menu.as_mut()
        && !menu.items.is_empty() {
            menu.idx = (menu.idx + 1) % menu.items.len();
    }
}

pub fn quickfix_menu_prev(ed: &mut crate::state::Editor) {
    if let Some(menu) = ed.quickfix_menu.as_mut()
        && !menu.items.is_empty() {
            menu.idx = if menu.idx == 0 { menu.items.len() - 1 } else { menu.idx - 1 };
    }
}

/// Apply the selected action's workspace edit and close the menu.
pub fn quickfix_menu_accept(ed: &mut crate::state::Editor) {
    let Some(menu) = ed.quickfix_menu.take() else { return };
    let Some(action) = menu.items.get(menu.idx) else { return };
    match &action.edit_json {
        Some(json) => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
                crate::semantic::edit_apply::apply_workspace_edit(ed, &v);
                ed.frame_dirty = true;
            }
        }
        None => {
            ed.show_message(crate::state::MessageKind::Info,
                format!("'{}' has no edit to apply", action.title));
        }
    }
}

pub fn quickfix_menu_close(ed: &mut crate::state::Editor) {
    ed.quickfix_menu = None;
}
