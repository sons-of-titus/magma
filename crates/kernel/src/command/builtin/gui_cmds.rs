//! GUI surface requests.  The keymap can only bind kernel commands, but
//! "open the file picker" or "show the find bar" is chrome, not editor
//! state.  These commands queue a request on `Editor::ui_requests`; the
//! frontend drains it each frame and opens the matching surface.  A
//! frontend that ignores them (headless, tests) is simply never asked.

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
    cmds.register_fn("gui-buffers", "Open the buffer switcher", vec![], |ed, _| {
        request(ed, "buffers")
    });
    cmds.register_fn("gui-command-palette", "Open the command palette", vec![], |ed, _| {
        request(ed, "commands")
    });
    cmds.register_fn("gui-goto-line", "Open the go-to-line prompt", vec![], |ed, _| {
        request(ed, "goto-line")
    });
    cmds.register_fn("gui-branches", "Open the branch switcher", vec![], |ed, _| {
        request(ed, "branches")
    });
}
