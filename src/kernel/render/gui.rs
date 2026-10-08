//! Graphical renderer using eframe / egui — native IDE layout.
//!
//! `GuiApp::update` composes the shell: status bar, docked tool windows,
//! tab bar, breadcrumb bar, tool strip, project sidebar, the Surface-based
//! editor pane, and floating dialogs/tool windows.  Input translation lives
//! in `gui_input`, editor-pane painting in `gui_editor_pane`.

use std::sync::{Arc, RwLock};

use eframe::egui::{self, FontFamily, FontId};
use tokio::sync::mpsc;

use crate::kernel::input;
use crate::kernel::runtime::{process_background_event, BackgroundEvent};
use crate::kernel::state::Editor;

use super::gui_breadcrumb::{BreadcrumbAction, BreadcrumbBar};
use super::gui_dialog::{DialogAction, DialogManager};
use super::gui_editor_pane::{render_editor_pane, EditorPaneAction, PanePaint};
use super::gui_fonts::apply_fonts_to_egui;
use super::gui_input;
use super::gui_layout::GuiLayout;
use super::gui_sidebar::{ProjectTree, SidebarState};
use super::gui_status::StatusBar;
use super::gui_tabs::{TabAction, TabBar};
use super::gui_theme::GuiTheme;
use super::gpu_atlas::GpuGlyphAtlas;
use super::tool_window::ToolWindowManager;

pub struct GuiApp {
    editor:          Arc<RwLock<Editor>>,
    font_size:       f32,
    layout:          GuiLayout,
    sidebar_state:   SidebarState,
    bg_receiver:     mpsc::UnboundedReceiver<BackgroundEvent>,
    atlas:           GpuGlyphAtlas,
    /// True once the atlas has been rasterized with real font data.
    atlas_rasterized: bool,
    /// True when `atlas_pixels` must be re-uploaded to the GPU this frame.
    atlas_dirty:     bool,
    target_format:   Option<eframe::wgpu::TextureFormat>,
    tool_windows:    ToolWindowManager,
    dialogs:         DialogManager,
    /// Active chrome palette; re-resolved from the `gui-theme` option.
    theme:           GuiTheme,
    /// False until the palette has been pushed to egui + the editor once.
    theme_synced:    bool,
    /// Render signature of the last painted frame; when unchanged there is
    /// nothing new to paint and no repaint is scheduled (§9.4).
    last_render_signature: u64,
    /// Cursor blink phase, flipped by the heartbeat while text input is
    /// accepted (§9.4: repaint driven by cursor blinking).
    blink_visible: bool,
    last_blink: std::time::Instant,
}

impl GuiApp {
    pub fn new(editor: Arc<RwLock<Editor>>) -> Self {
        let (_tx, rx) = mpsc::unbounded_channel();
        Self::init(editor, rx)
    }

    pub fn new_with_bg(editor: Arc<RwLock<Editor>>, bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>) -> Self {
        Self::init(editor, bg_receiver)
    }

    fn init(editor: Arc<RwLock<Editor>>, bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>) -> Self {
        // Install the bundled JetBrains Mono when no font was loaded from
        // config — the GPU atlas and egui chrome both depend on it.
        {
            let mut ed = editor.write().unwrap_or_else(|e| e.into_inner());
            if ed.font_config.loaded_fonts.is_empty() {
                ed.font_config.bootstrap_bundled();
                ed.font_changed = true;
            }
        }
        let (font_size, font_bytes) = {
            let ed = editor.read().unwrap_or_else(|e| e.into_inner());
            (ed.font_config.size, ed.font_config.loaded_fonts.values().next().cloned())
        };
        let mut atlas = GpuGlyphAtlas::new();
        let atlas_rasterized = if let Some(bytes) = font_bytes {
            atlas.rasterize(&bytes, font_size);
            true
        } else {
            false
        };
        Self {
            editor,
            font_size,
            layout: GuiLayout::default(),
            sidebar_state: SidebarState::default(),
            bg_receiver,
            atlas,
            atlas_rasterized,
            atlas_dirty: true,
            target_format: None,
            tool_windows: ToolWindowManager::new(),
            dialogs: DialogManager::new(),
            theme: GuiTheme::mocha(),
            theme_synced: false,
            last_render_signature: 0,
            blink_visible: true,
            last_blink: std::time::Instant::now(),
        }
    }
}

impl eframe::App for GuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // ── Background events ──────────────────────────────────────────────
        while let Ok(event) = self.bg_receiver.try_recv() {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            process_background_event(&mut ed, event);
        }

        // ── GPU init + font rebuild ────────────────────────────────────────
        if let Some(rs) = frame.wgpu_render_state() {
            if self.target_format.is_none() {
                self.target_format = Some(rs.target_format);
            }
            let (needs_rebuild, font_config) = {
                let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
                let changed = ed.font_changed;
                ed.font_changed = false;
                (changed, ed.font_config.clone())
            };
            if needs_rebuild {
                apply_fonts_to_egui(&ctx, &font_config);
                if let Some(bytes) = font_config.loaded_fonts.values().next() {
                    self.atlas.rasterize(bytes, self.font_size);
                    self.atlas_rasterized = true;
                    self.atlas_dirty = true;
                }
            }
        }

        // ── Font metrics (must precede input collection for wheel units) ───
        self.font_size = {
            let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
            ed.font_config.size
        };
        let font_id = FontId::new(self.font_size, FontFamily::Monospace);
        let (char_w, line_h) = ctx.fonts_mut(|f| (
            f.glyph_width(&font_id, 'M'),
            f.row_height(&font_id),
        ));

        // ── Key + mouse events ──────────────────────────────────────────────
        let (key_events, mut raw_mouse_events) = gui_input::collect(&ctx, line_h);
        {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            for key in &key_events {
                if key == "ctrl-q"  { ed.running = false; break; }
                if key == "ctrl-\\" { self.layout.sidebar_open = !self.layout.sidebar_open; continue; }
                if key == "ctrl-shift-l" {
                    let next = if GuiTheme::option_is_light(&ed.options) { "mocha" } else { "latte" };
                    ed.options.insert("gui-theme".to_string(), next.to_string());
                    continue;
                }
                if let Some(kind) = DialogManager::shortcut(key) {
                    self.dialogs.open_dialog(kind);
                    continue;
                }
                // An open dialog owns the keyboard; Escape closes it.
                if self.dialogs.is_open() {
                    if key == "esc" { self.dialogs.close(); }
                    continue;
                }
                input::dispatch_key(&mut ed, key);
            }
        }

        // ── Running check ─────────────────────────────────────────────────
        {
            let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
            if !ed.running {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }

        // ── Theme resolve + propagation ────────────────────────────────────
        {
            let next = {
                let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
                GuiTheme::from_options(&ed.options)
            };
            let changed = !self.theme_synced || next.light != self.theme.light;
            self.theme = next;
            if changed {
                self.theme.apply_egui(&ctx);
                let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
                self.theme.apply_editor(&mut ed);
                self.theme_synced = true;
            }
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
        let mut sidebar_file:      Option<String>           = None;
        let mut breadcrumb_action: Option<BreadcrumbAction> = None;

        // Status bar — outermost bottom panel.
        egui::containers::Panel::bottom("magma_status")
            .exact_size(status_h)
            .frame(egui::Frame::NONE.fill(self.theme.crust)
                .inner_margin(egui::Margin::symmetric(6, 0)))
            .show(ui, |ui| {
                let ed = ea.read().unwrap_or_else(|e| e.into_inner());
                StatusBar::show(ui, &ed, &self.theme);
            });

        // Docked-bottom tool windows — inside the status bar, outside the editor.
        {
            let ed = ea.read().unwrap_or_else(|e| e.into_inner());
            self.tool_windows.show_docked_bottom(ui, &ed, &self.theme);
        }

        // Tab bar — top panel.
        egui::containers::Panel::top("magma_tabs")
            .show(ui, |ui| {
                let ed = ea.read().unwrap_or_else(|e| e.into_inner());
                if let Some(a) = TabBar::show(ui, &ed, &self.theme) {
                    tab_action = Some(a);
                }
            });

        // Breadcrumb bar — below the tab bar, above the editor.
        if breadcrumb_on {
            egui::containers::Panel::top("magma_breadcrumb")
                .frame(egui::Frame::NONE.fill(self.theme.crust))
                .show(ui, |ui| {
                    let ed = ea.read().unwrap_or_else(|e| e.into_inner());
                    breadcrumb_action = BreadcrumbBar::show(ui, &ed, &self.theme);
                });
        }

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
                    sidebar_file = ProjectTree::show(ui, &ed, &mut self.sidebar_state, &self.theme);
                });
        }

        // Apply chrome actions before rendering the central editor pane.
        if let Some(action) = tab_action.take() {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            action.apply(&mut ed);
        }

        if let Some(path) = sidebar_file {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            DialogAction::OpenFile(path).apply(&mut ed);
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
        {
            let ed = ea.read().unwrap_or_else(|e| e.into_inner());
            let actions = self.dialogs.show(&ctx, &ed, &self.theme);
            if !actions.is_empty() {
                let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
                for action in actions {
                    action.apply(&mut ed);
                }
            }
        }

        // Floating tool windows — rendered on top of everything.
        {
            let ed = ea.read().unwrap_or_else(|e| e.into_inner());
            self.tool_windows.show_floating(&ctx, &ed, &self.theme);
        }

        // ── Repaint scheduling (§9.4) ──────────────────────────────────────
        // Input events wake egui natively; this tail handles everything
        // else.  A 400 ms heartbeat is the pump deadline for background
        // events (LSP, tasks, autosave) and the cursor-blink tick — one
        // cheap frame per interval, not a continuous rebuild.  An input-
        // or change-driven frame additionally repaints immediately.
        const BLINK_PERIOD: std::time::Duration = std::time::Duration::from_millis(400);
        let (signature, blink_due) = {
            let mut ed = ea.write().unwrap_or_else(|e| e.into_inner());
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

        // ── Window title follows the focused buffer ────────────────────────
        let title = {
            let ed = ea.read().unwrap_or_else(|e| e.into_inner());
            match ed.focused_buffer_key() {
                0 => "Magma".to_string(),
                key => match ed.buffers.get(key).map(|a| a.lock().unwrap().name.clone()) {
                    Some(name) => format!("{name} — Magma"),
                    None => "Magma".to_string(),
                },
            }
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }
}
