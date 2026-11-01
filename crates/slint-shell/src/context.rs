//! Context menus: the reusable right-click surface for tabs, the project
//! tree, and the editor.  The menu carries which object it was raised on so
//! `context-command` can act on it.

use crate::MenuItem;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ContextTarget {
    /// Tab at this strip index.
    Tab(usize),
    /// Tree row at this path.
    Tree(String),
    /// The editor pane.
    Editor,
}

fn item(label: &str, command: &str) -> MenuItem {
    MenuItem {
        label: label.into(),
        command: command.into(),
        separator: false,
    }
}

fn separator() -> MenuItem {
    MenuItem {
        label: "".into(),
        command: "".into(),
        separator: true,
    }
}

/// The menu rows for a target.  Commands are `context:*` verbs; the payload
/// lives in `ContextTarget` state, not in the command string.
pub(crate) fn items_for(target: &ContextTarget) -> Vec<MenuItem> {
    match target {
        ContextTarget::Tab(_) => vec![
            item("Close", "context:tab-close"),
            item("Close Others", "context:tab-close-others"),
            item("Close to the Right", "context:tab-close-right"),
            item("Close All", "context:tab-close-all"),
            separator(),
            item("Copy Path", "context:tab-copy-path"),
        ],
        ContextTarget::Tree(_) => vec![
            item("New File…", "context:tree-new-file"),
            item("New Folder…", "context:tree-new-folder"),
            separator(),
            item("Copy Path", "context:tree-copy-path"),
            item("Reveal in Finder", "context:tree-reveal"),
            separator(),
            item("Rename…", "context:tree-rename"),
            item("Delete", "context:tree-delete"),
        ],
        ContextTarget::Editor => vec![
            item("Cut", "editor-cut"),
            item("Copy", "editor-copy"),
            item("Paste", "put-after-cursor"),
            item("Select All", "select-all"),
            separator(),
            item("Find…", "gui-find"),
            item("Go to Definition", "goto-definition"),
        ],
    }
}
