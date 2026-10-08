//! Graphical shell — a two-window model.
//!
//! The primary window is the **welcome** surface (open folder, recents,
//! key hints).  Opening a project opens the **main window** — the editor
//! shell lives in its own viewport — and the welcome window hides.  Quitting
//! the editor returns to the welcome window; closing the welcome window
//! exits the app.
//!
//! Input translation lives in `gui_input`, editor-pane painting in
//! `gui_editor_pane`, and the welcome/editor behaviors in `gui_shell`.

use std::sync::{Arc, RwLock};

use eframe::egui;
use tokio::sync::mpsc;

use crate::kernel::runtime::BackgroundEvent;
use crate::kernel::state::Editor;

use super::gui_dialog::DialogManager;
use super::gui_layout::GuiLayout;
use super::gui_shell::{self, ShellRuntime};
use super::gui_sidebar_state::SidebarState;
use super::gui_theme::GuiTheme;
use super::gpu_atlas::GpuGlyphAtlas;
use super::tool_window::ToolWindowManager;

const EDITOR_VIEWPORT: &str = "magma_editor";

pub struct GuiApp {
    pub(crate) editor:          Arc<RwLock<Editor>>,
    pub(crate) font_size:       f32,
    pub(crate) layout:          GuiLayout,
    pub(crate) sidebar_state:   SidebarState,
    pub(crate) bg_receiver:     mpsc::UnboundedReceiver<BackgroundEvent>,
    pub(crate) atlas:           GpuGlyphAtlas,
    /// True once the atlas has been rasterized with real font data.
    pub(crate) atlas_rasterized: bool,
    /// True when `atlas_pixels` must be re-uploaded to the GPU this frame.
    pub(crate) atlas_dirty:     bool,
    pub(crate) target_format:   Option<eframe::wgpu::TextureFormat>,
    pub(crate) tool_windows:    ToolWindowManager,
    pub(crate) dialogs:         DialogManager,
    /// Active chrome palette; re-resolved from the `gui-theme` option.
    pub(crate) theme:           GuiTheme,
    /// False until the palette has been pushed to egui + the editor once.
    pub(crate) theme_synced:    bool,
    /// Render signature of the last painted frame; when unchanged there is
    /// nothing new to paint and no repaint is scheduled (§9.4).
    pub(crate) last_render_signature: u64,
    /// Cursor blink phase, flipped by the heartbeat while text input is
    /// accepted (§9.4: repaint driven by cursor blinking).
    pub(crate) blink_visible: bool,
    pub(crate) last_blink: std::time::Instant,
    /// Welcome flow, project watching, and the folder picker.
    pub(crate) shell: ShellRuntime,
    /// True once a project is open and the main window exists.
    pub(crate) editor_open: bool,
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
        let shell = ShellRuntime::new(&editor);
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
            shell,
            editor_open: false,
        }
    }
}

impl eframe::App for GuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // ── GPU init + font rebuild (shared by both windows) ──────────────
        if gui_shell::sync_gpu_and_fonts(&mut self.target_format, &mut self.atlas, &mut self.atlas_dirty, &self.editor, frame, &ctx) {
            self.atlas_rasterized = true;
            self.font_size = {
                let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
                ed.font_config.size
            };
        }
        gui_shell::sync_theme(&mut self.theme, &mut self.theme_synced, &ctx, &self.editor);

        // ── The welcome window is the whole primary surface ───────────────
        if !self.editor_open {
            let welcome_action = {
                let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
                self.shell.welcome.show(ui, &ed, &self.theme)
            };
            if let Some(action) = welcome_action {
                let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
                self.shell.handle_welcome_action(&mut ed, action);
            }
            {
                let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
                self.shell.consume_open_folder_request(&mut ed);
            }
            let project_open = {
                let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
                ed.project_manager.project.root.is_some()
            };
            if project_open {
                // Opening a project opens the main window and hides welcome.
                self.editor_open = true;
                self.theme_synced = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
            self.schedule_repaint(&ctx);
            return;
        }


        let editor_id = egui::ViewportId(egui::Id::new(EDITOR_VIEWPORT));
        ctx.show_viewport_immediate(editor_id,
            egui::ViewportBuilder::default()
                .with_title("Magma")
                .with_inner_size([1280.0, 800.0])
                .with_min_inner_size([400.0, 300.0]),
            |ui, _class| {
                let vp_ctx = ui.ctx().clone();
                self.editor_shell(&vp_ctx, ui);
            });
    }
}
