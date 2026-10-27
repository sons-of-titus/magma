//! Graphical shell — a two-window model.
//!
//! The primary window is the **welcome** surface: a JetBrains-style greeter
//! with a navigation rail (Projects, Customize, Learn), the New Project /
//! Open entry points, and the recent-projects list.  Opening a project opens
//! the **main window** — the editor shell lives in its own viewport — and
//! the welcome window hides.  Quitting the editor returns to the welcome
//! window; closing the welcome window exits the app.
//!
//! Input translation lives in `gui_input`, editor-pane painting in
//! `gui_editor_pane`, and the welcome/editor behaviors in `gui_shell`.
//!
//! GuiApp **owns** the editor (rework.md §1): the GUI loop is the single
//! foreground owner, and background threads speak through the event channel.

use eframe::egui;
use tokio::sync::mpsc;

use magma::kernel::runtime::BackgroundEvent;
use magma::kernel::state::Editor;

use super::gui_dialog::DialogManager;
use super::gui_layout::GuiLayout;
use super::gui_commit_tool::CommitState;
use super::gui_shell::{self, ShellRuntime};
use super::gui_sidebar_state::SidebarState;
use super::gui_theme::GuiTheme;
use super::gpu_atlas::GpuGlyphAtlas;
use super::tool_window::ToolWindowManager;

const EDITOR_VIEWPORT: &str = "magma_editor";

pub struct GuiApp {
    pub(crate) editor:          Editor,
    pub(crate) font_size:       f32,
    pub(crate) layout:          GuiLayout,
    pub(crate) sidebar_state:   SidebarState,
    /// Commit tool-window UI state (change lists, commit box, diff preview).
    pub(crate) commit_state:    CommitState,
    pub(crate) bg_receiver:     mpsc::UnboundedReceiver<BackgroundEvent>,
    pub(crate) atlas:           GpuGlyphAtlas,
    /// True once the atlas has been rasterized with real font data.
    pub(crate) atlas_rasterized: bool,
    /// Pixels-per-point the atlas was last rasterized at (F-35 HiDPI):
    /// a change re-rasterizes the atlas at device resolution.
    pub(crate) atlas_ppx: f32,
    /// True when `atlas_pixels` must be re-uploaded to the GPU this frame.
    pub(crate) atlas_dirty:     bool,
    pub(crate) target_format:   Option<eframe::wgpu::TextureFormat>,
    pub(crate) tool_windows:    ToolWindowManager,
    pub(crate) dialogs:         DialogManager,
    /// Active chrome palette; re-resolved from the `gui-theme` option.
    pub(crate) theme:           GuiTheme,
    /// False until the palette has been pushed to egui + the editor once.
    pub(crate) theme_synced:    bool,
    /// Last title pushed to the window.  Sending the command forces egui to
    /// schedule a repaint even when the title is unchanged, so it must only
    /// be sent when the text actually differs.
    pub(crate) last_window_title: String,
    /// Debug session seen last frame.  A session appearing is what opens the
    /// Debugger dock, however the session was started.
    pub(crate) last_debug_session: Option<u64>,
    /// Cursor blink phase, flipped by the heartbeat while text input is
    /// accepted (§9.4: repaint driven by cursor blinking).
    pub(crate) blink_visible: bool,
    pub(crate) last_blink: std::time::Instant,
    /// Welcome flow, project watching, and the folder picker.
    pub(crate) shell: ShellRuntime,
    /// True once a project is open and the main window exists.
    pub(crate) editor_open: bool,
    /// Scratch grid for the central editor pane, reused across frames.
    pub(crate) pane_surface: crate::render::surface::Surface,
    /// PTY output waker; consumed once by the repaint thread.
    pub(crate) pty_wake_rx: Option<std::sync::mpsc::Receiver<()>>,
    pub(crate) waker_started: bool,
    /// Sub-line wheel delta carried between events.
    pub(crate) wheel_remain: f32,
    /// Fractional horizontal wheel accumulator (mirrors `wheel_remain`).
    pub(crate) wheel_remain_x: f32,
    /// Active IME composition (R-15); rendered inline at the cursor and
    /// cleared when the IME commits or cancels.
    pub(crate) preedit: Option<super::gui_editor_pane::PreeditOverlay>,
}

impl GuiApp {
    pub fn new(editor: Editor) -> Self {
        let (_tx, rx) = mpsc::unbounded_channel();
        Self::init(editor, rx)
    }

    pub fn new_with_bg(editor: Editor, bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>) -> Self {
        Self::init(editor, bg_receiver)
    }

    fn init(mut editor: Editor, bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>) -> Self {
        // Install the default editor font when none was loaded from config:
        // the system monospace (Monaco on macOS) if present, else the bundled
        // JetBrains Mono.  The GPU atlas and egui chrome both depend on it.
        if editor.font_config.loaded_fonts.is_empty() {
            editor.font_config.bootstrap_default_font();
            editor.font_changed = true;
        }
        let (font_size, font_bytes) = (
            editor.font_config.size,
            editor.font_config.primary_font().cloned(),
        );
        let mut atlas = GpuGlyphAtlas::new();
        let atlas_rasterized = if let Some(bytes) = font_bytes {
            atlas.rasterize(&bytes, font_size);
            true
        } else {
            false
        };
    let shell = ShellRuntime::new(&mut editor);
    // Resolve the chrome palette before the editor is moved into `Self`.
    let theme = GuiTheme::from_options(&editor.options);
    // PTY output wakes the event loop through this channel instead of a
    // frame-rate poll in schedule_repaint.
    let (wake_tx, wake_rx) = std::sync::mpsc::channel();
    editor.io.terminals.pty_waker = Some(wake_tx);
    Self {
            editor,
            font_size,
            layout: GuiLayout::default(),
            sidebar_state: SidebarState::default(),
            commit_state: CommitState::default(),
            bg_receiver,
            atlas,
            atlas_rasterized,
            atlas_ppx: 1.0,
            atlas_dirty: true,
            target_format: None,
            tool_windows: ToolWindowManager::new(),
            dialogs: DialogManager::new(),
            theme,
            theme_synced: false,
            last_window_title: String::new(),
            last_debug_session: None,
            blink_visible: true,
            last_blink: std::time::Instant::now(),
            shell,
            editor_open: false,
            pane_surface: crate::render::surface::Surface::new(1, 1),
            pty_wake_rx: Some(wake_rx),
            waker_started: false,
            wheel_remain: 0.0,
            wheel_remain_x: 0.0,
            preedit: None,
        }
    }
}

impl eframe::App for GuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // ── GPU init + font rebuild (shared by both windows) ──────────────
        if gui_shell::sync_gpu_and_fonts(&mut self.target_format, &mut self.atlas, &mut self.atlas_dirty, &mut self.atlas_ppx, &mut self.editor, frame, &ctx) {
            self.atlas_rasterized = true;
            self.font_size = self.editor.font_config.size;
        }
        gui_shell::sync_theme(&mut self.theme, &mut self.theme_synced, &ctx, &mut self.editor);
        magma::kernel::icons::sync_option(&mut self.editor);

        // ── The welcome window is the whole primary surface ───────────────
        if !self.editor_open {
            let welcome_action = self.shell.welcome.show(ui, &self.editor, &self.theme);
            if let Some(action) = welcome_action {
                self.shell.handle_welcome_action(&mut self.editor, action);
            }
            self.shell.consume_folder_request(&mut self.editor);
            if self.editor.project_manager.project.root.is_some() {
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
