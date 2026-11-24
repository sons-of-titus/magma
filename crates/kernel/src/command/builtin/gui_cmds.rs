//! GUI surface requests.  The keymap can only bind kernel commands, but
//! "open the file picker" or "show the find bar" is chrome, not editor
//! state.  These commands queue a request on `Editor::ui_requests`; the
//! frontend drains it each frame and opens the matching surface.  A
//! frontend that ignores them (headless, tests) is simply never asked.

use std::collections::HashMap;

use crate::command::execute_command;
use crate::command::CommandResult;
use crate::state::Editor;

fn request(editor: &mut Editor, surface: &'static str) -> CommandResult {
    editor.ui_requests.push(surface.to_string());
    Ok(())
}

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("gui-find", "Open the find bar for the focused buffer", vec![], |ed, _| {
        request(ed, "find")
    });
    cmds.register_fn("gui-replace", "Open the find bar in replace mode", vec![], |ed, _| {
        request(ed, "replace")
    });
    cmds.register_fn("gui-project-find", "Open project search in the Find panel", vec![], |ed, _| {
        request(ed, "project-find")
    });
    cmds.register_fn("gui-goto-file", "Open the Go to File picker", vec![], |ed, _| {
        request(ed, "goto-file")
    });
    cmds.register_fn("gui-goto-symbol", "Open the Go to Symbol picker", vec![], |ed, _| {
        request(ed, "goto-symbol")
    });
    cmds.register_fn(
        "gui-picker",
        "Open a registered picker source by id",
        vec![
            crate::command::args::ArgSpec::new("id", crate::command::args::ArgType::String),
            crate::command::args::ArgSpec::optional(
                "query",
                crate::command::args::ArgType::String,
                crate::command::args::ArgValue::String(String::new()),
            ),
        ],
        |ed, args| {
            let id = args
                .get("id")
                .and_then(|a| a.as_string())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "gui-picker: source id required".to_string())?;
            if ed.picker_sources.get(&id).is_none() {
                return Err(format!("no picker source registered: {id}"));
            }
            let query = args
                .get("query")
                .and_then(|a| a.as_string())
                .unwrap_or_default();
            if query.is_empty() {
                ed.ui_requests.push(format!("picker-source:{id}"));
            } else {
                ed.ui_requests.push(format!("picker-source:{id}:{query}"));
            }
            Ok(())
        },
    );
    cmds.register_fn(
        "gui-goto-symbol-in-file",
        "Open the Go to Symbol in File picker",
        vec![],
        |ed, _| {
            if ed.picker_sources.get("document-symbols").is_none() {
                return Err("no symbol provider for this buffer".to_string());
            }
            ed.ui_requests.push("picker-source:document-symbols".to_string());
            Ok(())
        },
    );
    cmds.register_fn("gui-buffers", "Open the buffer switcher", vec![], |ed, _| {
        request(ed, "buffers")
    });
    cmds.register_fn("gui-command-palette", "Open the command palette", vec![], |ed, _| {
        request(ed, "commands")
    });
    cmds.register_fn("gui-goto-line", "Open the go-to-line prompt", vec![], |ed, _| {
        request(ed, "goto-line")
    });
    cmds.register_fn("gui-rename-symbol", "Rename the symbol at the cursor via LSP", vec![], |ed, _| {
        request(ed, "rename-symbol")
    });
    cmds.register_fn("gui-branches", "Open the branch switcher", vec![], |ed, _| {
        request(ed, "branches")
    });
    cmds.register_fn("hover-go-to-definition", "Go to definition of the hovered symbol", vec![], |ed, _| {
        let Some(card) = ed.hover.clone() else { return Ok(()); };
        if let Some(view) = ed.view_of_buffer_mut(card.buf_key) {
            view.cursor.offset = card.offset;
        }
        let args = HashMap::new();
        let has_lsp = ed
            .lsp
            .language_for_buffer(card.buf_key)
            .and_then(|lang| ed.lsp.get(&lang))
            .is_some();
        if has_lsp {
            let _ = execute_command(ed, "lsp-go-to-definition", &args);
        } else {
            let _ = execute_command(ed, "goto-definition", &args);
        }
        Ok(())
    });
}
