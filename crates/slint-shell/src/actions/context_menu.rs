//! Context menus: the right-click surface for tabs, the project tree, and
//! the editor.  The menu carries which object it was raised on so its
//! commands act on the right target; the shell's one overlay renders it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[cfg(not(target_os = "macos"))]
use kernel::command::args::ArgValue;
use kernel::command::execute_command;
use kernel::render::tabs;
use slint::ComponentHandle;

use super::picker::{self, PickerKind};
use crate::context::{self, ContextTarget};
use crate::{AppState, AppWindow, sync::refresh};

pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_tab_context(move |index, x, y| {
            let mut s = state.borrow_mut();
            s.context_menu = Some(ContextTarget::Tab(index.max(0) as usize));
            show(&weak, &mut s, x, y);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_tree_context(move |path, x, y| {
            let mut s = state.borrow_mut();
            s.context_menu = Some(ContextTarget::Tree(path.to_string()));
            s.context_path = path.to_string();
            show(&weak, &mut s, x, y);
        });
    }
    {
        let state = Rc::clone(&state);
        app.on_context_dismissed(move || {
            state.borrow_mut().context_menu = None;
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_context_command(move |cmd| {
            let mut s = state.borrow_mut();
            s.context_menu = None;
            run_command(&weak, &mut s, &cmd);
            refresh(&weak, &mut s);
        });
    }
}

/// The editor's right-click: raise the menu at the pointer after the mouse
/// event itself has been dispatched (cursor first, then the menu).
pub(super) fn raise_editor_menu(
    weak: &slint::Weak<AppWindow>,
    s: &mut AppState,
    x: f32,
    y: f32,
) {
    s.context_menu = Some(ContextTarget::Editor);
    show(weak, s, x, y);
}

fn show(weak: &slint::Weak<AppWindow>, s: &mut AppState, x: f32, y: f32) {
    let items = s
        .context_menu
        .as_ref()
        .map(context::items_for)
        .unwrap_or_default();
    if let Some(a) = weak.upgrade() {
        a.set_context_items(slint::ModelRc::new(slint::VecModel::from(items)));
        a.set_context_menu_x(x);
        a.set_context_menu_y(y);
        a.set_context_menu_visible(true);
    }
    s.editor.frame_dirty = true;
}

fn run_command(weak: &slint::Weak<AppWindow>, s: &mut AppState, cmd: &str) {
    let target = s.context_menu.clone();
    match cmd {
        "context:tab-close" | "context:tab-close-others" | "context:tab-close-right"
        | "context:tab-close-all" | "context:tab-copy-path" => {
            let index = match target {
                Some(ContextTarget::Tab(i)) => i,
                _ => return,
            };
            let all = tabs::collect_tabs(&s.editor);
            if cmd == "context:tab-copy-path" {
                if let Some(tab) = all.get(index) {
                    copy_buffer_path(s, tab.window_id);
                }
                return;
            }
            let close: Vec<usize> = match cmd {
                "context:tab-close" => vec![index],
                "context:tab-close-others" => (0..all.len()).filter(|i| *i != index).collect(),
                "context:tab-close-right" => (index + 1..all.len()).collect(),
                _ => (0..all.len()).collect(),
            };
            let mut closed = 0;
            for i in close.into_iter().rev() {
                if let Some(tab) = all.get(i) {
                    tabs::close_tab(&mut s.editor, tab.window_id);
                    closed += 1;
                }
            }
            s.message = format!("Closed {closed} tab(s)");
        }
        "context:tree-new-file" | "context:tree-new-folder" => {
            let dir = dir_of(s, &s.context_path);
            let kind = if cmd.ends_with("new-file") {
                PickerKind::NewFile(dir)
            } else {
                PickerKind::NewFolder(dir)
            };
            picker::open(weak, s, kind);
        }
        "context:tree-copy-path" => {
            let path = s.context_path.clone();
            copy_absolute(s, &path);
        }
        "context:tree-reveal" => {
            let path = absolute(s, &s.context_path);
            #[cfg(target_os = "macos")]
            let result = std::process::Command::new("open")
                .args(["-R", &path])
                .status()
                .map(|_| ())
                .map_err(|e| e.to_string());
            #[cfg(not(target_os = "macos"))]
            let result: Result<(), String> = Err("Reveal in Finder is macOS-only".into());
            match result {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
        }
        "context:tree-rename" => {
            let path = s.context_path.clone();
            let name = name_of(&path);
            picker::open(weak, s, PickerKind::Rename(path));
            s.picker_query = name.clone();
            if let Some(a) = weak.upgrade() {
                a.set_picker_query(name.into());
            }
            picker::preset(weak, s);
        }
        "context:tree-delete" => {
            let path = absolute(s, &s.context_path);
            #[cfg(target_os = "macos")]
            let result = trash_macos(&path);
            #[cfg(not(target_os = "macos"))]
            let result: Result<(), String> = {
                let mut args = HashMap::new();
                args.insert("path".to_string(), ArgValue::Path(path.clone()));
                execute_command(&mut s.editor, "delete-path", &args)
            };
            match result {
                Ok(()) => {
                    s.message = format!("Moved {} to Trash", name_of(&path));
                    s.tree_epoch = u64::MAX;
                }
                Err(e) => s.message = e,
            }
        }
        // Editor menu rows carry kernel command names directly.
        other => match execute_command(&mut s.editor, other, &HashMap::new()) {
            Ok(()) => s.message.clear(),
            Err(e) => s.message = e,
        },
    }
}

#[cfg(target_os = "macos")]
fn trash_macos(path: &str) -> Result<(), String> {
    let script = format!(
        "tell application \"Finder\" to delete (POSIX file \"{}\" as alias)",
        path.replace('\\', "\\\\").replace('"', "\\\"")
    );
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .status()
        .map(|_| ())
        .map_err(|e| format!("Cannot move to Trash: {e}"))
}

fn dir_of(s: &AppState, path: &str) -> String {
    if s.dirs.contains(path) {
        path.to_string()
    } else {
        std::path::Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

fn absolute(s: &AppState, rel: &str) -> String {
    match &s.editor.project_manager.project.root {
        Some(root) if !std::path::Path::new(rel).is_absolute() => {
            root.join(rel).to_string_lossy().into_owned()
        }
        _ => rel.to_string(),
    }
}

fn name_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

pub(super) fn copy_absolute(s: &mut AppState, rel: &str) {
    let path = absolute(s, rel);
    s.editor.clipboard.set_text(&path);
    s.message = "Path copied".into();
}

fn copy_buffer_path(s: &mut AppState, window_id: kernel::state::id::WindowId) {
    let path = s
        .editor
        .view_tree
        .buffer(window_id)
        .and_then(|key| s.editor.buffers.get(key))
        .and_then(|arc| arc.lock().unwrap_or_else(|e| e.into_inner()).path.clone());
    match path {
        Some(p) => {
            s.editor.clipboard.set_text(&p);
            s.message = "Path copied".into();
        }
        None => s.message = "This tab has no file on disk.".into(),
    }
}
