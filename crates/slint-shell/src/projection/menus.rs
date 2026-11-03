//! Menu projections: the native menu bar, the fallback popup menu, and the
//! contract test that the Navigate menu keeps its command palette entry.

use crate::MenuItem;

pub fn menu_title(title: &str) -> &'static str {
    match title {
        "Edit" => "Edit",
        "View" => "View",
        "Navigate" => "Navigate",
        "Run" => "Run",
        _ => "File",
    }
}

pub fn menu_items(title: &'static str) -> Vec<MenuItem> {
    let rows: &[(&str, &str)] = match title {
        "File" => &[
            ("Open Folder…", "open-project-dialog"),
            ("", ""),
            ("Save", "save-buffer"),
            ("Save All", "save-all"),
            ("", ""),
            ("Close Buffer", "close-buffer"),
            ("Quit", "quit"),
        ],
        "Edit" => &[
            ("Undo", "undo"),
            ("Redo", "redo"),
            ("", ""),
            ("Cut", "editor-cut"),
            ("Copy", "editor-copy"),
            ("Paste", "put-after-cursor"),
            ("Select All", "select-all"),
            ("", ""),
            ("Find…", "dialog:find"),
            ("Replace…", "dialog:replace"),
            ("Find in Project…", "dialog:project-find"),
            ("", ""),
            ("Go to Line…", "dialog:goto-line"),
        ],
        "View" => &[
            ("Project", "rail:0"),
            ("Commit", "rail:1"),
            ("Debugger", "rail:2"),
            ("", ""),
            ("Toggle Theme", "toggle-theme"),
            ("Settings…", "dialog:settings"),
        ],
        "Navigate" => &[
            ("Go to File…", "dialog:goto-file"),
            ("Go to Symbol…", "dialog:goto-symbol"),
            ("Switch Buffer…", "dialog:buffers"),
            ("Go to Line…", "dialog:goto-line"),
            ("", ""),
            ("Next Tab", "buffer-next"),
            ("Previous Tab", "buffer-prev"),
            ("", ""),
            ("Command Palette…", "dialog:run-command"),
        ],
        _ => &[
            ("Run", "run-config"),
            ("Debug", "debug-config"),
            ("Stop", "stop-config"),
            ("", ""),
            ("Refresh Version Control", "vc-refresh"),
        ],
    };
    rows.iter()
        .map(|(label, command)| MenuItem {
            label: (*label).into(),
            command: (*command).into(),
            separator: label.is_empty(),
            shortcut: String::new().into(),
        })
        .collect()
}

#[cfg(test)]
mod menu_tests {
    use super::*;

    #[test]
    fn navigate_menu_offers_run_command() {
        let items = menu_items("Navigate");
        assert!(
            items.iter().any(|i| i.command == "dialog:run-command"),
            "Navigate menu must expose Run Command…, got {:?}",
            items.iter().map(|i| i.label.clone()).collect::<Vec<_>>()
        );
    }
}
