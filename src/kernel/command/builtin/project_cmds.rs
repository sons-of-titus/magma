//! Project commands — open a folder as the active project, (re)index it.

use crate::kernel::state::Editor;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("open-project",
        "Open a folder as the active project and index its files",
        vec![crate::kernel::command::args::ArgSpec::new("path", crate::kernel::command::args::ArgType::Path)],
        |editor, args| {
            let path = args.get("path")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "Path required".to_string())?;
            if !std::path::Path::new(path).is_dir() {
                return Err(format!("Not a directory: {path}"));
            }
            crate::kernel::project::open_root(editor, path);
            crate::kernel::project::spawn_file_index(editor);
            Ok(())
        },
    );

    cmds.register_fn("index-project", "Re-index the active project's files",
        vec![],
        |editor, _args| {
            if editor.project_manager.project.root.is_none() {
                return Err("No project open".to_string());
            }
            crate::kernel::project::spawn_file_index(editor);
            Ok(())
        },
    );
}
