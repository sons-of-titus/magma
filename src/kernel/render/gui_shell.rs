//! Shell-side runtime behaviors split out of `gui.rs`: the welcome flow,
//! project watching with debounced re-index, and the native folder picker.
//! Rendering stays in `gui.rs`; this type owns the state those behaviors need.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use eframe::egui;

use crate::kernel::command::{args::ArgValue, execute_command};
use crate::kernel::state::{Editor, MessageKind};
use crate::kernel::storage::watcher::FileWatcher;

use super::gui_dialog::DialogAction;
use super::gui_sidebar_state::SidebarAction;
use super::gui_status::StatusAction;
use super::gui_welcome::{WelcomeAction, WelcomeScreen};

/// Apply chrome-initiated actions — the shell's write-lock boundary.  Every
/// failure lands in the status message; nothing is discarded silently.
pub(crate) fn apply_sidebar_actions(ed: &mut Editor, actions: Vec<SidebarAction>) {
    for action in actions {
        match action {
            SidebarAction::Open(path) => {
                DialogAction::OpenFile(path).apply(ed);
            }
            SidebarAction::CopyPath(path) => {
                ed.clipboard.set_text(&path);
                ed.show_message(MessageKind::Info, format!("Copied: {path}"));
            }
            SidebarAction::CreateFile { dir, name } => {
                let full = if dir.is_empty() { name } else { format!("{dir}/{name}") };
                let mut args = std::collections::HashMap::new();
                args.insert("path".to_string(), ArgValue::Path(full));
                if let Err(e) = execute_command(ed, "create-file", &args) {
                    ed.show_message(MessageKind::Error, e);
                }
                ed.project_manager.project.file_index_dirty = true;
            }
            SidebarAction::Rename { from, to } => {
                let mut args = std::collections::HashMap::new();
                args.insert("from".to_string(), ArgValue::Path(from));
                args.insert("to".to_string(), ArgValue::Path(to));
                if let Err(e) = execute_command(ed, "rename-path", &args) {
                    ed.show_message(MessageKind::Error, e);
                }
                ed.project_manager.project.file_index_dirty = true;
            }
            SidebarAction::Delete(path) => {
                let mut args = std::collections::HashMap::new();
                args.insert("path".to_string(), ArgValue::Path(path.clone()));
                if let Err(e) = execute_command(ed, "delete-path", &args) {
                    ed.show_message(MessageKind::Error, e);
                } else {
                    ed.show_message(MessageKind::Info, format!("Deleted: {path}"));
                }
                ed.project_manager.project.file_index_dirty = true;
            }
        }
    }
}

pub(crate) fn apply_status_action(ed: &mut Editor, action: StatusAction) {
    if action == StatusAction::OpenVc {
        let _ = execute_command(ed, "vc", &std::collections::HashMap::new());
    }
}

/// One-time GPU target capture + font (re)build after a font change.
/// Returns true when the glyph atlas was re-rasterized.
pub(crate) fn sync_gpu_and_fonts(
    target_format: &mut Option<eframe::wgpu::TextureFormat>,
    atlas: &mut super::gpu_atlas::GpuGlyphAtlas,
    atlas_dirty: &mut bool,
    editor: &std::sync::Arc<std::sync::RwLock<Editor>>,
    frame: &eframe::Frame,
    ctx: &egui::Context,
) -> bool {
    let Some(rs) = frame.wgpu_render_state() else { return false };
    if target_format.is_none() {
        *target_format = Some(rs.target_format);
    }
    let (needs_rebuild, font_config) = {
        let mut ed = editor.write().unwrap_or_else(|e| e.into_inner());
        let changed = ed.font_changed;
        ed.font_changed = false;
        (changed, ed.font_config.clone())
    };
    if needs_rebuild {
        super::gui_fonts::apply_fonts_to_egui(ctx, &font_config);
        if let Some(bytes) = font_config.loaded_fonts.values().next() {
            atlas.rasterize(bytes, font_config.size);
            *atlas_dirty = true;
            return true;
        }
    }
    false
}

/// Re-resolve the chrome palette from the `gui-theme` option and push it to
/// both render stacks when it changed.
pub(crate) fn sync_theme(
    theme: &mut super::gui_theme::GuiTheme,
    synced: &mut bool,
    ctx: &egui::Context,
    editor: &std::sync::Arc<std::sync::RwLock<Editor>>,
) {
    let next = {
        let ed = editor.read().unwrap_or_else(|e| e.into_inner());
        super::gui_theme::GuiTheme::from_options(&ed.options)
    };
    let changed = !*synced || next.light != theme.light;
    *theme = next;
    if changed {
        theme.apply_egui(ctx);
        let mut ed = editor.write().unwrap_or_else(|e| e.into_inner());
        theme.apply_editor(&mut ed);
        *synced = true;
    }
}

/// Drain background events delivered since the last frame.
pub(crate) fn drain_background_events(
    ed: &mut Editor,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::kernel::runtime::BackgroundEvent>,
) {
    while let Ok(event) = rx.try_recv() {
        crate::kernel::runtime::process_background_event(ed, event);
    }
}

/// The shell's key pre-dispatch: quit guard, chrome toggles, and dialog
/// shortcuts.  Returns true when the key was consumed; otherwise it goes to
/// the keymap.  This is the ONLY place the GUI takes keys from the keymap,
/// and only in normal/visual mode — text modes keep every chord.
pub(crate) fn dispatch_shell_key(
    ed: &mut Editor,
    key: &str,
    layout: &mut super::gui_layout::GuiLayout,
    dialogs: &mut super::gui_dialog::DialogManager,
) -> bool {
    if key == "ctrl-q" {
        if dialogs.request_quit(ed) {
            return true; // running=false; the shell closes this frame
        }
        return true; // dialog opened; key consumed
    }
    if key == "ctrl-\\" {
        layout.sidebar_open = !layout.sidebar_open;
        return true;
    }
    if key == "ctrl-shift-l" {
        let next = if super::gui_theme::GuiTheme::option_is_light(&ed.options) { "mocha" } else { "latte" };
        ed.options.insert("gui-theme".to_string(), next.to_string());
        return true;
    }
    if let Some(kind) = super::gui_dialog::DialogManager::shortcut(key)
        && ed.io.terminal_mode_buf.is_none() {
        dialogs.open_dialog(kind);
        return true;
    }
    // An open dialog owns the keyboard; Escape closes it.
    if dialogs.is_open() {
        if key == "esc" {
            dialogs.close();
        }
        return true;
    }
    false
}

pub(crate) struct ShellRuntime {
    pub welcome: WelcomeScreen,
    /// Set when the user asked for the native folder picker; consumed at the
    /// end of the frame (the modal blocks, so it must not run mid-layout).
    pending_open_folder: bool,
    /// Watches open files and the project root for external changes.
    file_watcher: Option<FileWatcher>,
    /// Canonical project root currently under recursive watch.
    watched_project: Option<String>,
    /// When `index-project` was last requested — debounces rescan storms.
    last_index_request: Option<Instant>,
}

impl ShellRuntime {
    pub(crate) fn new(editor: &Arc<RwLock<Editor>>) -> Self {
        let file_watcher = {
            let ed = editor.write().unwrap_or_else(|e| e.into_inner());
            let mut watcher = ed.background.as_ref()
                .and_then(|bg| FileWatcher::new(bg.sender.clone()).ok());
            if let Some(w) = watcher.as_mut() {
                for (_, arc) in ed.buffers.iter() {
                    let buf = arc.lock().unwrap();
                    if let Some(ref path) = buf.path {
                        w.watch(path);
                    }
                }
            }
            watcher
        };
        Self {
            welcome: WelcomeScreen::default(),
            pending_open_folder: false,
            file_watcher,
            watched_project: None,
            last_index_request: None,
        }
    }

    /// Watch the project root on first sight and rescan the index when the
    /// file watcher marks it dirty (debounced).
    pub(crate) fn maintain_project(&mut self, ed: &mut Editor) {
        let Some(root) = ed.project_manager.project.root.clone() else {
            self.watched_project = None;
            return;
        };
        let root_str = root.to_string_lossy().into_owned();
        if self.watched_project.as_deref() != Some(root_str.as_str()) {
            if let Some(w) = self.file_watcher.as_mut() {
                w.watch_project(&root_str);
            }
            self.watched_project = Some(root_str);
        }
        if ed.project_manager.project.file_index_dirty
            && self.last_index_request.is_none_or(|t| t.elapsed() >= Duration::from_secs(2)) {
            let _ = crate::kernel::command::execute_command(ed, "index-project", &std::collections::HashMap::new());
            self.last_index_request = Some(Instant::now());
        }
    }

    pub(crate) fn handle_welcome_action(&mut self, ed: &mut Editor, action: WelcomeAction) {
        match action {
            WelcomeAction::OpenFolder => self.pending_open_folder = true,
            WelcomeAction::OpenProject(path) => {
                let mut args = std::collections::HashMap::new();
                args.insert("path".to_string(), ArgValue::Path(path));
                if let Err(e) = crate::kernel::command::execute_command(ed, "open-project", &args) {
                    ed.show_message(MessageKind::Error, e);
                }
            }
        }
    }

    /// Run the native folder picker if a click requested it.  The modal
    /// blocks the frame, so this runs after layout, outside widget closures.
    pub(crate) fn consume_open_folder_request(&mut self, ed: &mut Editor) {
        if !self.pending_open_folder {
            return;
        }
        self.pending_open_folder = false;
        let Some(dir) = rfd::FileDialog::new()
            .set_title("Open Folder")
            .pick_folder() else { return };
        let mut args = std::collections::HashMap::new();
        args.insert("path".to_string(),
            ArgValue::Path(dir.to_string_lossy().into_owned()));
        if let Err(e) = crate::kernel::command::execute_command(ed, "open-project", &args) {
            ed.show_message(MessageKind::Error, e);
        }
    }
}

/// The window title follows the focused buffer.
pub(crate) fn window_title(ed: &Editor) -> String {
    match ed.focused_buffer_key() {
        0 => "Magma".to_string(),
        key => match ed.buffers.get(key).map(|a| a.lock().unwrap().name.clone()) {
            Some(name) => format!("{name} — Magma"),
            None => "Magma".to_string(),
        },
    }
}
