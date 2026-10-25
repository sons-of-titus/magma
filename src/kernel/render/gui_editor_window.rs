//! The main editor window — the editor shell rendered into its own
//! viewport, opened by `open-project` from the welcome window.  Split from
//! `gui.rs` (welcome + window dispatch) at the window boundary.
//!
//! GuiApp owns the editor: every block below borrows `self.editor` directly.
//! Repaint scheduling is driven by `Editor::frame_dirty` — bumped by every
//! mutating entry point (input dispatch, background events, chrome actions)
//! — plus the cursor-blink heartbeat.  There is no per-frame polling of
//! buffer state.  The chrome panels themselves live in `gui_dock::show_panels`.

use std::collections::HashMap;

use eframe::egui::{self, FontFamily, FontId};

use crate::kernel::command::execute_command;
use crate::kernel::input;
use crate::kernel::state::{Editor, MessageKind};

use super::gui_breadcrumb::BreadcrumbAction;
use super::gui_commit_actions;
use super::gui_dock;
use super::gui_editor_pane::{render_editor_pane, EditorPaneAction, PanePaint};
use super::gui_input;
use super::gui_layout::LeftTool;
use super::gui_menu::MenuAction;
use super::gui_shell::{self, window_title};
use super::gui_status::StatusAction;
use super::gui_theme::GuiTheme;

use super::gui::GuiApp;

impl GuiApp {
    /// The editor shell — every panel, the central editor pane, dialogs and
    /// tool windows — rendered into the main window's viewport.
    pub(crate) fn editor_shell(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // PTY output repaints: one blocking thread, woken by the terminal
        // reader — no frame-rate polling while a shell sits idle.
        if !self.waker_started {
            self.waker_started = true;
            if let Some(rx) = self.pty_wake_rx.take() {
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    while rx.recv().is_ok() {
                        ctx.request_repaint();
                    }
                });
            }
        }

        let (char_w, line_h) = {
            let font_id = FontId::new(self.font_size, FontFamily::Monospace);
            ctx.fonts_mut(|f| (
                f.glyph_width(&font_id, 'M'),
                f.row_height(&font_id),
            ))
        };

        // ── Background events ──────────────────────────────────────────────
        gui_shell::drain_background_events(&mut self.editor, &mut self.bg_receiver);
        // Chrome actions (sidebar open, dialogs) queue editor events
        // (buffer-created drives syntax highlighting and LSP attach);
        // unlike key input, nothing else would drain them this frame.
        self.editor.dispatch_events_scoped();

        // `:vc` asks for the Version Control tool window; reveal it before the
        // panels are built so it appears in the same frame.
        if self.editor.vc.request_open {
            self.editor.vc.request_open = false;
            self.layout.left_tool = LeftTool::Commit;
            self.layout.sidebar_open = true;
        }

        // ── Key + mouse + IME events ───────────────────────────────────────
        let (key_events, mut raw_mouse_events, ime_events) =
            gui_input::collect(ctx, line_h, &mut self.wheel_remain, &mut self.wheel_remain_x);
        for ime in ime_events {
            match ime {
                gui_input::ImeInput::Preedit { text, cursor_char } => {
                    self.preedit = if text.is_empty() {
                        None
                    } else {
                        Some(super::gui_editor_pane::PreeditOverlay { text, cursor_char })
                    };
                    self.editor.frame_dirty = true;
                }
                gui_input::ImeInput::Commit(text) => {
                    self.preedit = None;
                    input::dispatch_key(&mut self.editor, &text);
                }
            }
        }
        for key in &key_events {
            if gui_shell::dispatch_shell_key(&mut self.editor, key, &mut self.layout, &mut self.dialogs) {
                continue;
            }
            input::dispatch_key(&mut self.editor, key);
        }

        // ── Window close (X button) routes through the quit guard ────────
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.dialogs.request_quit(&mut self.editor) {
                persist_session(&self.editor);
                self.close_editor(ctx);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }

        // ── Running check: quitting closes the editor, returns to welcome ─
        if !self.editor.running {
            persist_session(&self.editor);
            self.close_editor(ctx);
            return;
        }

        // ── Project watching + debounced re-index ──────────────────────────
        self.shell.maintain_project(&mut self.editor);

        // ── A new debug session opens the Debugger dock, however it started ──
        let session_id = self.editor.debug.active_session().map(|s| s.id);
        if session_id.is_some() && session_id != self.last_debug_session {
            self.layout.left_tool = LeftTool::Debug;
            self.layout.sidebar_open = true;
        }
        self.last_debug_session = session_id;

        // ── Chrome panels (menu, status, docks, rails, tabs, toolbar) ──────
        let out = gui_dock::show_panels(self, ui, line_h);

        // Apply chrome actions before rendering the central editor pane.
        if let Some(action) = out.tab {
            action.apply(&mut self.editor);
        }
        if !out.sidebar.is_empty() {
            gui_shell::apply_sidebar_actions(&mut self.editor, out.sidebar);
        }
        if let Some(action) = out.status {
            apply_status_action(&mut self.editor, &mut self.layout, action);
        }
        if !out.menu.is_empty() {
            self.apply_menu_actions(out.menu);
        }
        if !out.commit.is_empty() {
            gui_commit_actions::apply_commit_actions(&mut self.editor, out.commit);
        }
        if !out.run.is_empty() {
            gui_dock::apply_run_actions(
                &mut self.editor, &mut self.tool_windows, &mut self.layout, &mut self.dialogs, out.run);
        }
        if let Some(tool) = out.left_clicked {
            self.layout.left_tool = tool;
            self.layout.sidebar_open = true;
            if tool == LeftTool::Commit {
                let _ = execute_command(&mut self.editor, "vc-refresh", &HashMap::new())
                    .or_else(|_| execute_command(&mut self.editor, "vc", &HashMap::new()));
            }
        }
        if let Some(BreadcrumbAction::RevealInSidebar { rel_dir }) = out.breadcrumb {
            self.layout.sidebar_open = true;
            self.layout.left_tool = LeftTool::Project;
            let parts: Vec<&str> = rel_dir.split('/').filter(|s| !s.is_empty()).collect();
            for i in 1..=parts.len() {
                self.sidebar_state.expanded_dirs.insert(parts[..i].join("/"));
            }
            self.sidebar_state.selected = Some(rel_dir);
        }
        let mut tool_actions = out.tool;

        // ── Central panel: surface-based editor + scrollbar/minimap ───────
        // The buffer is CPU-rendered through egui's text rasterizer by
        // default (crisper, handles any glyph); the batched GPU quad path
        // is available via MAGMA_GPU_RENDER=1.
        let use_gpu = self.target_format.is_some()
            && self.atlas_rasterized
            && std::env::var_os("MAGMA_GPU_RENDER").is_some();
        let target_fmt = self.target_format;

        // Drain the dirty flag exactly once per frame.
        let atlas_pixels_upload = if self.atlas_dirty {
            self.atlas_dirty = false;
            Some(self.atlas.generate_atlas_pixels().to_vec())
        } else {
            None
        };

        let mut pane_action: Option<EditorPaneAction> = None;
        let mut paint = PanePaint {
            editor:        &mut self.editor,
            surface:       &mut self.pane_surface,
            atlas:         &self.atlas,
            use_gpu,
            target_format: target_fmt,
            atlas_pixels:  atlas_pixels_upload,
            theme:         &self.theme,
            preedit:       self.preedit.clone(),
        };
        egui::containers::CentralPanel::no_frame()
            .frame(egui::Frame::NONE.fill(self.theme.base))
            .show(ui, |ui| {
                pane_action = render_editor_pane(ui, &mut paint, char_w, line_h, &mut raw_mouse_events);
            });
        drop(paint);

        if let Some(action) = pane_action.take() {
            gui_dock::apply_pane_action(&mut self.editor, action);
        }

        // ── Modal dialogs ─────────────────────────────────────────────────
        let dialog_actions = self.dialogs.show(ctx, &self.editor, &self.theme);
        if !dialog_actions.is_empty() {
            for action in dialog_actions {
                action.apply(&mut self.editor);
            }
        }

        // Floating tool windows — rendered on top of everything.
        tool_actions.extend(self.tool_windows.show_floating(ctx, &mut self.editor, &self.theme));
        if !tool_actions.is_empty() {
            gui_shell::apply_tool_actions(&mut self.editor, tool_actions);
        }

        // ── Repaint scheduling (§9.4) ──────────────────────────────────────
        self.schedule_repaint(ctx);

        // ── Window title follows the focused buffer ────────────────────────
        // Every viewport command forces a repaint, so an unchanged title
        // must not be re-sent or the shell repaints forever at idle.
        let title = window_title(&self.editor);
        if self.last_window_title != title {
            self.last_window_title = title.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
    }

    fn apply_menu_actions(&mut self, actions: Vec<MenuAction>) {
        for action in actions {
            match action {
                MenuAction::Command(cmd) => {
                    if let Err(e) = execute_command(&mut self.editor, cmd, &HashMap::new()) {
                        self.editor.show_message(MessageKind::Error, e);
                    }
                }
                MenuAction::OpenDialog(kind) => self.dialogs.open_dialog(kind),
                MenuAction::ShowLeft(tool) => {
                    self.layout.left_tool = tool;
                    self.layout.sidebar_open = true;
                }
                MenuAction::ToggleTheme => {
                    let next = if GuiTheme::option_is_light(&self.editor.options) {
                        "intellij-dark"
                    } else {
                        "intellij-light"
                    };
                    self.editor.options.insert("gui-theme".to_string(), next.to_string());
                    self.editor.frame_dirty = true;
                }
            }
        }
    }

    /// Whether the caret should animate (and therefore whether the blink
    /// heartbeat is armed).  Only a focused, text-accepting surface blinks:
    /// an unfocused or non-text surface must arm nothing, or the editor
    /// repaints the whole UI every 400 ms while parked.
    pub(crate) fn caret_blinks(focused: bool, accepts_text: bool) -> bool {
        focused && accepts_text
    }

    pub(crate) fn schedule_repaint(&mut self, ctx: &egui::Context) {
        const BLINK_PERIOD: std::time::Duration = std::time::Duration::from_millis(400);

        // The caret blinks only while the window is focused.  An unfocused or
        // non-text surface has nothing to animate, so it must not arm the
        // heartbeat: the heartbeat is the ONLY idle repaint source, and gating
        // it here is what lets a parked window truly sleep instead of
        // repainting the whole UI every 400 ms.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        let blink_active = Self::caret_blinks(focused, self.editor.editor_mode.accepts_text);

        if blink_active {
            if self.last_blink.elapsed() >= BLINK_PERIOD {
                self.blink_visible = !self.blink_visible;
                self.last_blink = std::time::Instant::now();
            }
        } else {
            // A frozen caret stays solid — there is no deadline to honour.
            self.blink_visible = true;
        }
        self.editor.cursor_visible = self.blink_visible;

        // A pending atlas upload must not wake the welcome surface: nothing
        // there consumes it, and the editor pane needs the flag to still be
        // set on its first frame.
        let atlas_pending = self.atlas_dirty && self.editor_open;
        let repaint_now = self.editor.frame_dirty || atlas_pending;
        self.editor.frame_dirty = false;
        if repaint_now {
            ctx.request_repaint();
        }
        // Re-arm only while the caret is animating.  The wake itself renders
        // the new phase, so a flip needs no *immediate* repaint on top of it —
        // requesting one doubled the idle frame count.
        if blink_active {
            ctx.request_repaint_after(BLINK_PERIOD);
        }
    }

    /// Close the main window and bring the welcome window back.
    pub(crate) fn close_editor(&mut self, ctx: &egui::Context) {
        gui_shell::close_editor_session(&mut self.editor);
        self.editor_open = false;
        ctx.send_viewport_cmd_to(
            egui::ViewportId::ROOT,
            egui::ViewportCommand::Visible(true),
        );
        ctx.send_viewport_cmd_to(
            egui::ViewportId::ROOT,
            egui::ViewportCommand::Focus,
        );
    }
}

/// Apply a status-bar request; the VC pill switches the left dock to Commit.
fn apply_status_action(ed: &mut Editor, layout: &mut super::gui_layout::GuiLayout, action: StatusAction) {
    if action == StatusAction::OpenVc {
        layout.left_tool = LeftTool::Commit;
        layout.sidebar_open = true;
        let _ = execute_command(ed, "vc-refresh", &HashMap::new())
            .or_else(|_| execute_command(ed, "vc", &HashMap::new()));
    }
}

/// Persist history + marks on editor shutdown; the project session joins
/// them when a project is open.  A clean exit also discards this run's
/// crash-recovery snapshots — nothing unsaved remains (R-16).
fn persist_session(ed: &Editor) {
    crate::kernel::state::persist::save_session(ed);
    crate::kernel::session::save_if_project(ed);
    if let Some(dir) = ed.io.crash_dir.clone() {
        std::thread::spawn(move || {
            crate::kernel::storage::crash_recovery::clear_session(&dir);
        });
    }
}
