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
    /// Database explorer node, addressed by its node key.
    DbNode(String),
}

/// Context menu for a database explorer node.  The node's own kind lives
/// in its key (`/db/schema/tables/name`), so the menu shapes itself there.
fn db_items(key: &str) -> Vec<MenuItem> {
    let parts: Vec<&str> = key.split('/').filter(|p| !p.is_empty()).collect();
    match parts.len() {
        0 | 1 => vec![
            item("New Query Console", "context:db-console"),
            item("Refresh", "context:db-refresh-node"),
            separator(),
            item("Disconnect", "context:db-disconnect"),
            item("Properties…", "context:db-properties"),
        ],
        2 | 3 => vec![
            item("New Query Console", "context:db-console"),
            item("Copy Name", "context:db-copy-name"),
            separator(),
            item("Refresh", "context:db-refresh-node"),
        ],
        4 => {
            let kind = parts.get(2).copied().unwrap_or("");
            let is_object = kind == "tables" || kind == "views";
            let mut items = Vec::new();
            if is_object {
                items.push(item("Jump to Data", "context:db-open-table"));
                items.push(item("View Structure", "context:db-open-structure"));
                items.push(item("Select Top 100 Rows", "context:db-select-top"));
                items.push(separator());
            }
            items.push(item("Copy Name", "context:db-copy-name"));
            if is_object {
                items.push(item("Copy Qualified Name", "context:db-copy-qualified"));
            }
            items.push(separator());
            items.push(item("Refresh", "context:db-refresh-node"));
            items
        }
        _ => {
            let mut items = Vec::new();
            if parts.get(4) == Some(&"fks") {
                items.push(item("Go to Referenced Table", "context:db-follow-fk"));
                items.push(separator());
            }
            items.push(item("Copy Name", "context:db-copy-name"));
            items
        }
    }
}

fn item(label: &str, command: &str) -> MenuItem {
    MenuItem {
        label: label.into(),
        command: command.into(),
        separator: false,
        shortcut: String::new().into(),
    }
}

fn item_sc(label: &str, command: &str, shortcut: &str) -> MenuItem {
    MenuItem {
        label: label.into(),
        command: command.into(),
        separator: false,
        shortcut: shortcut.into(),
    }
}

fn separator() -> MenuItem {
    MenuItem {
        label: "".into(),
        command: "".into(),
        separator: true,
        shortcut: String::new().into(),
    }
}

/// macOS renders the ctrl chords as command chords; elsewhere the plain
/// ctrl names are what the keyboard shows.
#[cfg(target_os = "macos")]
fn pretty(key: &str) -> String {
    key.replace("ctrl-shift", "⇧⌘")
        .replace("ctrl", "⌘")
        .replace("meta-return", "⌥↩")
        .replace("meta-k", "⌥K")
        .replace("meta-left", "⌥←")
        .replace("meta-right", "⌥→")
        .replace("shift", "⇧")
        .replace("return", "↩")
        .replace("left", "←")
        .replace("right", "→")
        .replace("up", "↑")
        .replace("down", "↓")
        .replace("page-up", "⇞")
        .replace("page-down", "⇟")
        .replace("home", "↖")
        .replace("end", "↘")
        .replace("space", "Space")
        .replace('-', "")
        .to_uppercase()
}

#[cfg(not(target_os = "macos"))]
fn pretty(key: &str) -> String {
    key.split('-')
        .map(|part| match part {
            "meta" => "Alt".to_string(),
            "return" => "Enter".to_string(),
            "page-up" => "PgUp".to_string(),
            "page-down" => "PgDn".to_string(),
            other => {
                let mut c = other.chars();
                match c.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// The menu rows for a target.  Commands are `context:*` verbs; the payload
/// lives in `ContextTarget` state, not in the command string.  The editor
/// menu reflects the selection: with one, cut/copy act on it; without,
/// they act on the line under the cursor (the kernel commands' fallback).
pub(crate) fn items_for(target: &ContextTarget, has_selection: bool) -> Vec<MenuItem> {
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
        ContextTarget::DbNode(key) => db_items(key),
        ContextTarget::Editor => {
            let (cut, copy) = if has_selection {
                ("Cut", "Copy")
            } else {
                ("Cut Line", "Copy Line")
            };
            vec![
                item_sc(cut, "editor-cut", &pretty("ctrl-x")),
                item_sc(copy, "editor-copy", &pretty("ctrl-c")),
                item_sc("Paste", "put-after-cursor", &pretty("ctrl-v")),
                item_sc("Select All", "select-all", &pretty("ctrl-a")),
                separator(),
                item_sc("Find…", "gui-find", &pretty("ctrl-f")),
                item_sc("Replace…", "gui-replace", &pretty("ctrl-h")),
                item_sc("Find in Project…", "gui-project-find", &pretty("ctrl-shift-f")),
                item_sc("Go to Line…", "gui-goto-line", &pretty("ctrl-l")),
                item_sc("Command Palette…", "gui-command-palette", &pretty("ctrl-shift-p")),
                separator(),
                item_sc("Go to Definition", "goto-definition", &pretty("ctrl-]")),
                item_sc("Code Actions…", "lsp-code-actions", &pretty("meta-return")),
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_menu_adapts_labels_to_the_selection() {
        let with = items_for(&ContextTarget::Editor, true);
        let without = items_for(&ContextTarget::Editor, false);
        assert_eq!(with.len(), without.len(), "same row count either way");
        assert_eq!(with[0].label, "Cut");
        assert_eq!(without[0].label, "Cut Line");
        assert_eq!(without[1].label, "Copy Line");
        assert!(with.iter().all(|i| !i.separator || i.label.is_empty()));
    }

    #[test]
    fn editor_menu_rows_carry_shortcut_hints() {
        let items = items_for(&ContextTarget::Editor, true);
        let copy = items.iter().find(|i| i.command == "editor-copy").unwrap();
        assert!(!copy.shortcut.is_empty(), "copy shows its chord");
        assert!(
            items.iter().find(|i| i.command == "put-after-cursor").unwrap().shortcut.len() > 0
        );
        assert!(items.iter().find(|i| i.separator).is_some());
    }

    #[test]
    fn tab_and_tree_menus_are_unchanged_in_shape() {
        let tabs = items_for(&ContextTarget::Tab(0), false);
        assert!(tabs.iter().any(|i| i.command == "context:tab-close"));
        let tree = items_for(&ContextTarget::Tree("a/b".into()), false);
        assert!(tree.iter().any(|i| i.command == "context:tree-rename"));
    }

    #[test]
    fn shortcut_hints_render_on_both_platforms() {
        let find = pretty("ctrl-shift-f");
        assert!(find.contains('F'), "{find} must name the letter");
        let actions = pretty("meta-return");
        assert!(!actions.is_empty(), "{actions} must be non-empty");
        let paste = pretty("ctrl-v");
        assert!(paste.contains('V'), "{paste} must name the letter");
    }
}
