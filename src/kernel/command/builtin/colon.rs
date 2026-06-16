//! Fallback `:command` dispatcher used when Janet is not loaded.

use crate::kernel::command::CommandResult;
use crate::kernel::state::Editor;

use super::helpers::get_current_buffer_id;

/// Execute a `:command` string (e.g. "w", "q", "wq", "e path").
/// This is the default Rust implementation; when Janet is loaded, the Janet
/// implementation (in colon_mode.janet via command/define) takes precedence.
pub(super) fn execute_colon_command(editor: &mut Editor, cmd: &str) -> CommandResult {
    let (verb, rest) = cmd.split_once(' ')
        .map(|(v, r)| (v, r.trim()))
        .unwrap_or((cmd, ""));

    match verb {
        "w" | "write" => {
            let buf_id = get_current_buffer_id(editor);
            let buf = editor.buffers.get_mut(buf_id)
                .ok_or_else(|| "No buffer".to_string())?;
            let path = if rest.is_empty() {
                buf.path.clone().ok_or_else(|| "No file path (use :w <path>)".to_string())?
            } else {
                rest.to_string()
            };
            let content = buf.slice(0, buf.len());
            editor.fs.write(&path, &content).map_err(|e| e.to_string())?;
            buf.path = Some(path);
            buf.mark_saved();
            Ok(())
        }
        "q" | "quit" | "q!" => {
            editor.running = false;
            Ok(())
        }
        "wq" | "x" => {
            let buf_id = get_current_buffer_id(editor);
            let buf = editor.buffers.get_mut(buf_id)
                .ok_or_else(|| "No buffer".to_string())?;
            let path = buf.path.clone()
                .ok_or_else(|| "No file path (use :w <path> first)".to_string())?;
            let content = buf.slice(0, buf.len());
            editor.fs.write(&path, &content).map_err(|e| e.to_string())?;
            buf.mark_saved();
            editor.running = false;
            Ok(())
        }
        "e" | "edit" => {
            if rest.is_empty() { return Err(":e requires a path".to_string()); }
            let content = editor.fs.read(rest).map_err(|e| e.to_string())?;
            let name = std::path::Path::new(rest)
                .file_name().map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| rest.to_string());
            let id = editor.allocate_buffer_id();
            let mut buf = crate::kernel::text_engine::Buffer::from_string(
                crate::kernel::state::id::BufferId(id), &name, &content,
            );
            buf.path = Some(rest.to_string());
            let entry = editor.buffers.vacant_entry();
            let key = entry.key();
            entry.insert(buf);
            if let Some(win) = editor.windows.focused_window_mut() {
                win.buffer_id = Some(key);
            }
            Ok(())
        }
        "set" => {
            let parts: Vec<&str> = rest.split(|c: char| c.is_whitespace()).filter(|s| !s.is_empty()).collect();
            for part in parts {
                if let Some((k, v)) = part.split_once('=') {
                    editor.options.insert(k.to_string(), v.to_string());
                } else {
                    editor.options.insert(part.to_string(), String::new());
                }
            }
            Ok(())
        }
        "s" => {
            if rest.is_empty() { return Err("Usage: :s/old/new/".to_string()); }
            let chars: Vec<char> = rest.chars().collect();
            if chars.len() < 2 { return Err("Invalid syntax".to_string()); }
            let sep = chars[0];
            let mut parts = Vec::new();
            let mut current = String::new();
            let mut esc = false;
            for &c in chars[1..].iter() {
                if esc { current.push(c); esc = false; continue; }
                if c == '\\' { esc = true; current.push(c); continue; }
                if c == sep { parts.push(current.clone()); current.clear(); continue; }
                current.push(c);
            }
            if !current.is_empty() { parts.push(current); }
            if parts.len() < 2 { return Err("Need at least /pattern/replacement/".to_string()); }
            let pattern = parts[0].clone();
            let replacement = parts[1].clone();
            let flags = if parts.len() > 2 { parts[2].clone() } else { String::new() };
            let global = flags.contains('g');
            let buf_id = get_current_buffer_id(editor);
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                let text = buf.slice(0, buf.len());
                let result = if global {
                    text.replace(&pattern, &replacement)
                } else {
                    if let Some(idx) = text[0..].find(&pattern) {
                        let mut r = text[..idx].to_string();
                        r.push_str(&replacement);
                        r.push_str(&text[idx+pattern.len()..]);
                        r
                    } else {
                        text
                    }
                };
                buf.replace(0, buf.len(), &result);
            }
            Ok(())
        }
        "!" => {
            if rest.is_empty() { return Err(":! requires a shell command".to_string()); }
            crate::kernel::task::run_shell_command(editor, rest)?;
            Ok(())
        }
        "make" => {
            crate::kernel::task::run_make(editor, rest)?;
            Ok(())
        }
        "cn" | "cnext" => {
            crate::kernel::task::quickfix_next(editor)
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        "cp" | "cprev" | "cprevious" => {
            crate::kernel::task::quickfix_prev(editor)
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        "r" | "read" => {
            if rest.is_empty() { return Err(":r requires a file path".to_string()); }
            let content = editor.fs.read(rest).map_err(|e| e.to_string())?;
            let buf_id = get_current_buffer_id(editor);
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                let pos = buf.cursor();
                buf.insert(pos, &content);
            }
            Ok(())
        }
        "tab" | "tabedit" => {
            if rest.is_empty() { return Err(":tabe requires a path".to_string()); }
            let content = editor.fs.read(rest).map_err(|e| e.to_string())?;
            let name = std::path::Path::new(rest)
                .file_name().map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| rest.to_string());
            let id = editor.allocate_buffer_id();
            let mut buf = crate::kernel::text_engine::Buffer::from_string(
                crate::kernel::state::id::BufferId(id), &name, &content,
            );
            buf.path = Some(rest.to_string());
            let entry = editor.buffers.vacant_entry();
            let key = entry.key();
            entry.insert(buf);
            if let Some(win) = editor.windows.focused_window_mut() {
                win.buffer_id = Some(key);
            }
            Ok(())
        }
        _ => Err(format!("Unknown command: {}", verb)),
    }
}
