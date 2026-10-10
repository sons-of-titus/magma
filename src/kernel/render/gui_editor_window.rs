//! The main editor window — the editor shell rendered into its own
//! viewport, opened by `open-project` from the welcome window.  Split from
//! `gui.rs` (welcome + window dispatch) at the window boundary.
//!
//! GuiApp owns the editor: every block below borrows `self.editor` directly.
//! Repaint scheduling is driven by `Editor::frame_dirty` — bumped by every
//! mutating entry point (input dispatch, background events, chrome actions)
//! — plus the cursor-blink heartbeat.  There is no per-frame polling of
//! buffer state.

use eframe::egui::{self, FontFamily, FontId};

use crate::kernel::input;
use crate::kernel::state::Editor;

use super::gui_breadcrumb::{BreadcrumbAction, BreadcrumbBar};
use super::gui_dialog::DialogAction;
use super::gui_editor_pane::{render_editor_pane, EditorPaneAction, PanePaint};
use super::gui_input;
use super::gui_shell::{self, window_title};
use super::gui_sidebar::ProjectTree;
use super::gui_sidebar_state::SidebarAction;
use super::gui_status::{StatusBar, StatusAction};
use super::gui_tabs::{TabAction, TabBar};

use super::gui::GuiApp;

impl GuiApp {
    /// The editor shell — every panel, the central editor pane, dialogs and
    /// tool windows — rendered into the main window's viewport.
    pub(crate) fn editor_shell(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
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

        // ── Key + mouse events ──────────────────────────────────────────────
        let (key_events, mut raw_mouse_events) = gui_input::collect(ctx, line_h);
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

        // ── Panel layout ──────────────────────────────────────────────────
        let sidebar_open   = self.layout.sidebar_open;
        let sidebar_width  = self.layout.sidebar_width;
        let breadcrumb_on  = self.editor.options.get("breadcrumb-enabled").map(|v| v.as_str()) != Some("false");
        let status_h       = line_h + 8.0;
        let mut tab_action:        Option<TabAction>        = None;
        let mut sidebar_actions:   Vec<SidebarAction>       = Vec::new();
        let mut breadcrumb_action: Option<BreadcrumbAction> = None;

        // Status bar — outermost bottom panel.
        let mut status_action: Option<StatusAction> = None;
        egui::containers::Panel::bottom("magma_status")
            .exact_size(status_h)
            .frame(egui::Frame::NONE.fill(self.theme.crust)
                .inner_margin(egui::Margin::symmetric(6, 0)))
            .show(ui, |ui| {
                status_action = StatusBar::show(ui, &self.editor, &self.theme);
            });
        if let Some(action) = status_action {
            gui_shell::apply_status_action(&mut self.editor, action);
        }

        // The terminal dock follows session focus: spawning or focusing a
        // terminal opens the dock (entry 0); closing stays visible until
        // the user toggles it away.
        if self.editor.io.terminals.focused.is_some()
            && let Some(entry) = self.tool_windows.entries.first()
            && !entry.visible {
                self.tool_windows.toggle(super::tool_window::TERMINAL_TOOL);
            }

        // Docked-bottom tool windows — inside the status bar, outside the editor.
        let mut tool_actions = self.tool_windows.show_docked_bottom(ui, &mut self.editor, &self.theme);

        // Navigation band — tabs and breadcrumb share one top panel with one
        // background; the tab strip's own baseline separates them.
        egui::containers::Panel::top("magma_nav")
            .frame(egui::Frame::NONE.fill(self.theme.crust))
            .show(ui, |ui| {
                if let Some(a) = TabBar::show(ui, &self.editor, &self.theme) {
                    tab_action = Some(a);
                }
                if breadcrumb_on {
                    breadcrumb_action = BreadcrumbBar::show(ui, &self.editor, &self.theme);
                }
            });

        // Tool window icon strip — outermost left strip (28 px wide).
        let mut toggled_tool: Option<usize> = None;
        egui::containers::Panel::left("magma_tool_strip")
            .exact_size(28.0)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(self.theme.crust)
                .inner_margin(egui::Margin::same(0)))
            .show(ui, |ui| {
                toggled_tool = self.tool_windows.show_sidebar_edge(ui, &self.theme);
            });
        if let Some(idx) = toggled_tool {
            self.tool_windows.toggle(idx);
        }

        // Project tree sidebar.
        if sidebar_open {
            egui::containers::Panel::left("magma_sidebar")
                .default_size(sidebar_width)
                .resizable(true)
                .show(ui, |ui| {
                    sidebar_actions = ProjectTree::show(ui, &self.editor, &mut self.sidebar_state, &self.theme);
                });
        }

        // Apply chrome actions before rendering the central editor pane.
        if let Some(action) = tab_action.take() {
            action.apply(&mut self.editor);
        }

        if !sidebar_actions.is_empty() {
            gui_shell::apply_sidebar_actions(&mut self.editor, sidebar_actions);
        }

        if let Some(BreadcrumbAction::RevealInSidebar { rel_dir }) = breadcrumb_action.take() {
            self.layout.sidebar_open = true;
            let parts: Vec<&str> = rel_dir.split('/').filter(|s| !s.is_empty()).collect();
            for i in 1..=parts.len() {
                self.sidebar_state.expanded_dirs.insert(parts[..i].join("/"));
            }
            self.sidebar_state.selected = Some(rel_dir);
        }

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
            atlas:         &self.atlas,
            use_gpu,
            target_format: target_fmt,
            atlas_pixels:  atlas_pixels_upload,
            theme:         &self.theme,
        };
        egui::containers::CentralPanel::no_frame()
            .frame(egui::Frame::NONE.fill(self.theme.base))
            .show(ui, |ui| {
                pane_action = render_editor_pane(ui, &mut paint, char_w, line_h, &mut raw_mouse_events);
            });
        drop(paint);

        if let Some(EditorPaneAction::GotoLine(line)) = pane_action.take() {
            DialogAction::GotoLine(line).apply(&mut self.editor);
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

    pub(crate) fn schedule_repaint(&mut self, ctx: &egui::Context) {
        const BLINK_PERIOD: std::time::Duration = std::time::Duration::from_millis(400);
        const TERMINAL_FRAME: std::time::Duration = std::time::Duration::from_millis(16);
        let (dirty, blinked) = {
            let mut blinked = false;
            if self.editor.editor_mode.accepts_text && self.last_blink.elapsed() >= BLINK_PERIOD {
                self.blink_visible = !self.blink_visible;
                self.last_blink = std::time::Instant::now();
                blinked = true;
            } else if !self.editor.editor_mode.accepts_text {
                self.blink_visible = true;
            }
            self.editor.cursor_visible = self.blink_visible;
            (self.editor.frame_dirty, blinked)
        };
        // A pending atlas upload must not wake the welcome surface: nothing
        // there consumes it, and the editor pane needs the flag to still be
        // set on its first frame.
        let atlas_pending = self.atlas_dirty && self.editor_open;
        let repaint_now = dirty || blinked || atlas_pending;
        self.editor.frame_dirty = false;
        if repaint_now {
            ctx.request_repaint();
        }
        if self.editor.io.terminals.sessions.is_empty()
            || !self.tool_windows.entries.first().is_some_and(|e| e.visible) {
            ctx.request_repaint_after(BLINK_PERIOD);
        } else {
            // A live PTY produces output between frames; poll at frame rate
            // so keystroke echo and command output land without perceptible
            // delay instead of waiting out the blink period.
            ctx.request_repaint_after(TERMINAL_FRAME);
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

/// Persist history + marks on editor shutdown.
fn persist_session(ed: &Editor) {
    crate::kernel::state::persist::save_session(ed);
}
