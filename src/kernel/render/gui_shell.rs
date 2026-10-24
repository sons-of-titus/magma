//! Shell-side runtime behaviors split out of `gui.rs`: the welcome flow,
//! project watching with debounced re-index, and the native folder picker.
//! Rendering stays in `gui.rs`; this type owns the state those behaviors need.

use std::time::{Duration, Instant};

use eframe::egui;

use crate::kernel::command::{args::ArgValue, execute_command};
use crate::kernel::state::{Editor, MessageKind};
use crate::kernel::storage::watcher::FileWatcher;

use super::gui_dialog::DialogAction;
use super::gui_sidebar_state::SidebarAction;
use super::gui_status::StatusAction;
use super::gui_welcome::{WelcomeAction, WelcomeScreen};
use super::tool_window::ToolAction;

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

/// Teardown when the editor window goes away (quit, guarded close): the
/// active project ends with it and terminal sessions are killed.  Without
/// this the welcome screen's `project.root` gate would immediately re-open
/// the editor instead of returning to the welcome flow.
pub(crate) fn close_editor_session(ed: &mut Editor) {
    // Snapshot the project session before the root goes away.
    crate::kernel::session::save_if_project(ed);
    if ed.project_manager.project.root.take().is_some() {
        ed.project_manager.current_project = None;
        ed.events.emit_typed(
            crate::kernel::event::keys::events::PROJECT_CLOSED,
            crate::kernel::event::payload::EmptyPayload,
        );
    }
    ed.io.terminals.close_all();
    ed.debug.disconnect_all();
    let _ = ed.index.flush();
}

/// Apply tool-window requests (navigation hits, terminal lifecycle) at the
/// shell's write-lock boundary.
pub(crate) fn apply_tool_actions(ed: &mut Editor, actions: Vec<ToolAction>) {    use crate::kernel::terminal;

    for action in actions {
        match action {
            ToolAction::Goto { path, line } => {
                DialogAction::OpenFileAtLine { path, line }.apply(ed);
            }
            ToolAction::TerminalFocus(id) => {
                terminal::set_focused(ed, Some(id));
            }
            ToolAction::TerminalNew => {
                if let Err(e) = execute_command(ed, "terminal-start", &std::collections::HashMap::new()) {
                    ed.show_message(MessageKind::Error, e);
                }
            }
            ToolAction::TerminalClose(id) => {
                ed.io.terminals.close(id);
                if ed.io.terminals.focused.is_none() {
                    terminal::set_focused(ed, None);
                }
            }
            ToolAction::TerminalRename { id, name } => {
                ed.io.terminals.rename(id, &name);
            }
            ToolAction::TerminalResize { id, cols, rows } => {
                let _ = ed.io.terminals.resize(id, cols, rows);
            }
            ToolAction::AgentSubmit(prompt) => {
                match crate::kernel::agent::loop_::start_session(ed, &prompt) {
                    Ok(id) => {
                        ed.show_message(MessageKind::Info,
                            format!("agent session {id} started"));
                    }
                    Err(e) => ed.show_message(MessageKind::Error, e),
                }
            }
            ToolAction::AgentStop(id) => {
                if ed.agent.request_stop(id) {
                    ed.show_message(MessageKind::Info,
                        "agent will stop after its current step");
                }
            }
            ToolAction::DebugCmd(cmd) => {
                if let Err(e) = execute_command(ed, cmd, &std::collections::HashMap::new()) {
                    ed.show_message(MessageKind::Error, e);
                }
            }
            ToolAction::DebugEval(expr) => {
                if let Err(e) = crate::kernel::command::execute_command_line(
                    ed, &format!("dbg-eval {expr}")) {
                    ed.show_message(MessageKind::Error, e);
                }
            }
            ToolAction::DebugExpand(reference) => {
                if let Some(sid) = ed.debug.active_session {
                    crate::kernel::runtime_dap::request_variables(ed, sid, reference);
                }
            }
            ToolAction::WatchAdd(expr) => {
                if !ed.debug.watches.contains(&expr) {
                    ed.debug.watches.push(expr.clone());
                    ed.show_message(MessageKind::Info, format!("Watching {expr} — evaluated next stop"));
                }
            }
            ToolAction::WatchRemove(expr) => {
                ed.debug.watches.retain(|w| w != &expr);
                for session in ed.debug.sessions.values_mut() {
                    session.watch_results = vec![None; ed.debug.watches.len()];
                }
            }
            ToolAction::BreakpointToggle { file, line } => {
                let enabled = ed.debug.breakpoints_for(&file).iter()
                    .find(|bp| bp.line == line)
                    .map(|bp| bp.enabled)
                    .unwrap_or(false);
                crate::kernel::debug::set_breakpoint_enabled(ed, &file, line, !enabled);
            }
            ToolAction::BreakpointRemove { file, line } => {
                crate::kernel::debug::remove_breakpoints_at(ed, &file, line);
            }
        }
    }
}

/// One-time GPU target capture + font (re)build after a font change.
/// Returns true when the glyph atlas was re-rasterized.
pub(crate) fn sync_gpu_and_fonts(
    target_format: &mut Option<eframe::wgpu::TextureFormat>,
    atlas: &mut super::gpu_atlas::GpuGlyphAtlas,
    atlas_dirty: &mut bool,
    ed: &mut Editor,
    frame: &eframe::Frame,
    ctx: &egui::Context,
) -> bool {
    let Some(rs) = frame.wgpu_render_state() else { return false };
    if target_format.is_none() {
        *target_format = Some(rs.target_format);
    }
    let changed = ed.font_changed;
    ed.font_changed = false;
    if changed {
        // Glyph metrics feed the row-layout cache; drop it so rows re-lay
        // out against the new widths (R-12).
        ed.row_layouts.clear();
        let font_config = ed.font_config.clone();
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
    ed: &mut Editor,
) {
    let next = super::gui_theme::GuiTheme::from_options(&ed.options);
    let changed = !*synced || next.light != theme.light;
    *theme = next;
    if changed {
        theme.apply_egui(ctx);
        theme.apply_editor(ed);
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
        // sync_theme ran at frame start; flag a repaint so the new palette
        // lands on the next frame.
        ed.frame_dirty = true;
        return true;
    }
    if let Some(kind) = super::gui_dialog::DialogManager::shortcut(key)
        && ed.io.terminals.focused.is_none() {
        dialogs.open_dialog(kind);
        return true;
    }
    // An open dialog owns the keyboard; Escape closes it — but in the
    // picker's argument mode it steps back to the list first.
    if dialogs.is_open() {
        if key == "esc" && !dialogs.picker.go_back() {
            dialogs.close();
            ed.frame_dirty = true;
        }
        return true;
    }
    false
}

/// Where a welcome pick-folder request should point the native dialog.
#[derive(Clone, Copy, PartialEq)]
enum FolderRequest {
    Open,
    Create,
}

pub(crate) struct ShellRuntime {
    pub welcome: WelcomeScreen,
    /// Set when the user asked for the native folder picker; consumed at the
    /// end of the frame (the modal blocks, so it must not run mid-layout).
    pending_folder: Option<FolderRequest>,
    /// Watches open files and the project root for external changes.
    file_watcher: Option<FileWatcher>,
    /// Canonical project root currently under recursive watch.
    watched_project: Option<String>,
    /// When `index-project` was last requested — debounces rescan storms.
    last_index_request: Option<Instant>,
}

impl ShellRuntime {
    pub(crate) fn new(ed: &mut Editor) -> Self {
        let file_watcher = ed.background.as_ref()
            .and_then(|bg| FileWatcher::new(bg.sender.clone()).ok())
            .map(|mut w| {
                for (_, arc) in ed.buffers.iter() {
                    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(ref path) = buf.path {
                        w.watch(path);
                    }
                }
                w
            });
        Self {
            welcome: WelcomeScreen::default(),
            pending_folder: None,
            file_watcher,
            watched_project: None,
            last_index_request: None,
        }
    }

    /// Watch the project root on first sight and rescan the index when the
    /// file watcher marks it dirty (debounced).
    pub(crate) fn maintain_project(&mut self, ed: &mut Editor) {
        // Files opened from outside the project root have no recursive
        // watch covering them — keep every open buffer path watched so
        // external changes still reach the reload path.
        if let Some(w) = self.file_watcher.as_mut() {
            let paths: Vec<String> = ed.buffers.iter()
                .filter_map(|(_, arc)| {
                    arc.lock().unwrap_or_else(|e| e.into_inner()).path.clone()
                })
                .collect();
            for path in paths {
                if !w.is_watched(&path) {
                    w.watch(&path);
                }
            }
        }
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
        ed.frame_dirty = true;
        match action {
            WelcomeAction::OpenFolder => self.pending_folder = Some(FolderRequest::Open),
            WelcomeAction::NewProject => self.pending_folder = Some(FolderRequest::Create),
            WelcomeAction::OpenProject(path) => {
                let mut args = std::collections::HashMap::new();
                args.insert("path".to_string(), ArgValue::Path(path));
                if let Err(e) = crate::kernel::command::execute_command(ed, "open-project", &args) {
                    ed.show_message(MessageKind::Error, e);
                }
            }
            WelcomeAction::RemoveRecent(path) => {
                crate::kernel::project::remove_recent(ed, &path);
            }
            WelcomeAction::SetTheme(name) => {
                ed.options.insert("gui-theme".to_string(), name.to_string());
            }
            WelcomeAction::FontSize(step) => {
                ed.font_config.size = (ed.font_config.size + step).clamp(8.0, 40.0);
                ed.font_changed = true;
            }
        }
    }

    /// Run the native folder picker if a click requested it.  The modal
    /// blocks the frame, so this runs after layout, outside widget closures.
    pub(crate) fn consume_folder_request(&mut self, ed: &mut Editor) {
        let Some(kind) = self.pending_folder.take() else { return };
        let mut dialog = rfd::FileDialog::new().set_title(match kind {
            FolderRequest::Open => "Open Folder",
            FolderRequest::Create => "New Project",
        });
        if kind == FolderRequest::Create {
            dialog = dialog.set_can_create_directories(true);
        }
        let Some(dir) = dialog.pick_folder() else { return };
        let path = dir.to_string_lossy().into_owned();
        if kind == FolderRequest::Create
            && !dir.is_dir()
            && let Err(e) = std::fs::create_dir_all(&dir) {
                ed.show_message(MessageKind::Error, format!("Could not create {path}: {e}"));
                return;
            }
        let mut args = std::collections::HashMap::new();
        args.insert("path".to_string(), ArgValue::Path(path));
        if let Err(e) = crate::kernel::command::execute_command(ed, "open-project", &args) {
            ed.show_message(MessageKind::Error, e);
        }
    }
}

/// The window title follows the focused buffer.
pub(crate) fn window_title(ed: &Editor) -> String {
    match ed.focused_buffer_key() {
        0 => "Magma".to_string(),
        key => match ed.buffers.get(key).map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).name.clone()) {
            Some(name) => format!("{name} — Magma"),
            None => "Magma".to_string(),
        },
    }
}
