//! The main editor window — the editor shell rendered into its own
//! viewport, opened by `open-project` from the welcome window.  Split from
//! `gui.rs` (welcome + window dispatch) at the window boundary.

use std::sync::Arc;

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
use super::tool_window::ToolAction;

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
        {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            gui_shell::drain_background_events(&mut ed, &mut self.bg_receiver);
        }

        // ── Key + mouse events ──────────────────────────────────────────────
        let (key_events, mut raw_mouse_events) = gui_input::collect(ctx, line_h);
        {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            for key in &key_events {
                if gui_shell::dispatch_shell_key(&mut ed, key, &mut self.layout, &mut self.dialogs) {
                    continue;
                }
                input::dispatch_key(&mut ed, key);
            }
        }

        // ── Window close (X button) routes through the quit guard ────────
        if ctx.input(|i| i.viewport().close_requested()) {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            if self.dialogs.request_quit(&mut ed) {
                persist_session(&ed);
                drop(ed);
                self.close_editor(ctx);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }

        // ── Running check: quitting closes the editor, returns to welcome ─
        let should_close = {
            let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
            if !ed.running {
                persist_session(&ed);
                true
            } else {
                false
            }
        };
        if should_close {
            self.close_editor(ctx);
            return;
        }

        // ── Project watching + debounced re-index ──────────────────────────
        {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            self.shell.maintain_project(&mut ed);
        }

        // ── Panel layout ──────────────────────────────────────────────────
        let ea             = Arc::clone(&self.editor);
        let sidebar_open   = self.layout.sidebar_open;
        let sidebar_width  = self.layout.sidebar_width;
        let breadcrumb_on  = {
            let ed = ea.read().unwrap_or_else(|e| e.into_inner());
            ed.options.get("breadcrumb-enabled").map(|v| v.as_str()) != Some("false")
        };
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
                let ed = ea.read().unwrap_or_else(|e| e.into_inner());
                status_action = StatusBar::show(ui, &ed, &self.theme);
            });
        if let Some(action) = status_action {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            gui_shell::apply_status_action(&mut ed, action);
        }

        // Docked-bottom tool windows — inside the status bar, outside the editor.
        let mut tool_actions = {
            let ed = ea.read().unwrap_or_else(|e| e.into_inner());
            self.tool_windows.show_docked_bottom(ui, &ed, &self.theme)
        };

        // Navigation band — tabs and breadcrumb share one top panel with one
        // background, instead of two stacked chrome bars.
        egui::containers::Panel::top("magma_nav")
            .frame(egui::Frame::NONE.fill(self.theme.crust))
            .show(ui, |ui| {
                let ed = ea.read().unwrap_or_else(|e| e.into_inner());
                if let Some(a) = TabBar::show(ui, &ed, &self.theme) {
                    tab_action = Some(a);
                }
                if breadcrumb_on {
                    ui.separator();
                    breadcrumb_action = BreadcrumbBar::show(ui, &ed, &self.theme);
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
                    let ed = ea.read().unwrap_or_else(|e| e.into_inner());
                    sidebar_actions = ProjectTree::show(ui, &ed, &mut self.sidebar_state, &self.theme);
                });
        }

        // Apply chrome actions before rendering the central editor pane.
        if let Some(action) = tab_action.take() {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            action.apply(&mut ed);
        }

        if !sidebar_actions.is_empty() {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            gui_shell::apply_sidebar_actions(&mut ed, sidebar_actions);
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
        let paint = PanePaint {
            editor:        &self.editor,
            atlas:         &self.atlas,
            use_gpu,
            target_format: target_fmt,
            atlas_pixels:  atlas_pixels_upload,
            theme:         &self.theme,
        };
        egui::containers::CentralPanel::no_frame()
            .frame(egui::Frame::NONE.fill(self.theme.base))
            .show(ui, |ui| {
                pane_action = render_editor_pane(ui, &paint, char_w, line_h, &mut raw_mouse_events);
            });

        if let Some(EditorPaneAction::GotoLine(line)) = pane_action.take() {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            DialogAction::GotoLine(line).apply(&mut ed);
        }

        // ── Modal dialogs ─────────────────────────────────────────────────
        // The read guard must DROP before the write lock is taken: a writer
        // behind our own reader on this thread would block forever (the
        // "palette hangs on every selection" deadlock).
        let dialog_actions = {
            let ed = ea.read().unwrap_or_else(|e| e.into_inner());
            self.dialogs.show(ctx, &ed, &self.theme)
        };
        if !dialog_actions.is_empty() {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            for action in dialog_actions {
                action.apply(&mut ed);
            }
        }

        // Floating tool windows — rendered on top of everything.
        {
            let ed = ea.read().unwrap_or_else(|e| e.into_inner());
            tool_actions.extend(self.tool_windows.show_floating(ctx, &ed, &self.theme));
        }
        if !tool_actions.is_empty() {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            for action in tool_actions {
                let ToolAction::Goto { path, line } = action;
                DialogAction::OpenFileAtLine { path, line }.apply(&mut ed);
            }
        }

        // ── Repaint scheduling (§9.4) ──────────────────────────────────────
        self.schedule_repaint(ctx);

        // ── Window title follows the focused buffer ────────────────────────
        let title = {
            let ed = ea.read().unwrap_or_else(|e| e.into_inner());
            window_title(&ed)
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    pub(crate) fn schedule_repaint(&mut self, ctx: &egui::Context) {
        const BLINK_PERIOD: std::time::Duration = std::time::Duration::from_millis(400);
        let (signature, blink_due) = {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            if ed.editor_mode.accepts_text && self.last_blink.elapsed() >= BLINK_PERIOD {
                self.blink_visible = !self.blink_visible;
                self.last_blink = std::time::Instant::now();
            } else if !ed.editor_mode.accepts_text {
                self.blink_visible = true;
            }
            ed.cursor_visible = self.blink_visible;
            (ed.render_signature(), self.last_blink.elapsed() >= BLINK_PERIOD)
        };
        let repaint_now = signature != self.last_render_signature
            || blink_due
            || self.atlas_dirty;
        self.last_render_signature = signature;
        if repaint_now {
            ctx.request_repaint();
        }
        ctx.request_repaint_after(BLINK_PERIOD);
    }

    /// Close the main window and bring the welcome window back.
    pub(crate) fn close_editor(&mut self, ctx: &egui::Context) {
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
