//! File commands (open-file-at-cursor).

use crate::kernel::state::Editor;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("open-file-at-cursor", "Open the file path under the cursor",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let path = {
                let view = editor.views.get(&buf_id)
                    .ok_or_else(|| "No buffer".to_string())?;
                let line_num = current_line(view);
                let buf = view.buffer.lock().unwrap();
                buf.line(line_num)
                    .ok_or_else(|| "Could not read line".to_string())?
                    .trim()
                    .to_string()
            };

            if path.is_empty() {
                return Err("No text on current line".to_string());
            }

            let content = editor.fs.read(&path)
                .map_err(|e| format!("Can't open '{}': {}", path, e))?;
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            let key = editor.create_buffer_from_str(&name, &content);
            if let Some(arc) = editor.buffers.get(key) {
                arc.lock().unwrap().path = Some(path);
            }
            if let Some(win) = editor.view_tree.focused_window_mut() {
                win.buffer_id = Some(key);
            }
            Ok(())
        },
    );
}
