//! Project commands — open a folder as the active project, (re)index it.

use crate::state::Editor;

/// Expand a leading `~` to `$HOME`; anything else passes through unchanged.
fn expand_tilde(path: &str) -> String {
    let is_tilde = path == "~" || path.starts_with("~/");
    if is_tilde && let Ok(home) = std::env::var("HOME") {
        return format!("{home}{}", &path[1..]);
    }
    path.to_string()
}

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "open-project",
        "Open a folder as the active project and index its files",
        vec![crate::command::args::ArgSpec::new(
            "path",
            crate::command::args::ArgType::Path,
        )],
        |editor, args| {
            let path = args
                .get("path")
                .and_then(|a| a.as_string())
                .map(expand_tilde)
                .ok_or_else(|| "Path required".to_string())?;
            if !std::path::Path::new(&path).is_dir() {
                return Err(format!("Not a directory: {path}"));
            }
            // Switching projects snapshots the outgoing session first, so
            // each project reopens exactly as it was left.
            crate::session::save_if_project(editor);
            crate::project::open_root(editor, &path);
            crate::project::spawn_file_index(editor);
            crate::session::restore(editor, std::path::Path::new(&path));
            // An active project implies a live session: reopening from the
            // welcome window must re-arm `running`, which a prior quit left
            // off and would otherwise bounce the editor straight back.
            editor.running = true;
            Ok(())
        },
    );

    cmds.register_fn(
        "index-project",
        "Re-index the active project's files",
        vec![],
        |editor, _args| {
            if editor.project_manager.project.root.is_none() {
                return Err("No project open".to_string());
            }
            crate::project::spawn_file_index(editor);
            Ok(())
        },
    );
}
