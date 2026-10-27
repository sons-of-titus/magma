//! Slint frontend — a declarative shell over the kernel's `Surface` pipeline.
//!
//! The UI is split into reusable components (`ui/components/`) over a single
//! `Theme` global.  This file owns the event loop and the action boundary:
//! components emit commands, the shell applies them through the same registry
//! every other frontend uses.

mod actions;
mod input;
mod new_palettes;
mod palettes;
mod pane;
mod projection;
mod sync;
mod theme;

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use kernel::boot::{self, BootOptions};
use kernel::project::tree::FileNode;
use kernel::render::gpu_atlas::GpuGlyphAtlas;
use kernel::render::surface::Surface;
use kernel::runtime::{BackgroundEvent, process_background_event};
use kernel::state::Editor;
use kernel::workspace::{self, WorkspaceRuntime};
#[cfg(feature = "janet")]
use scripting;
slint::include_modules!();

const STATUS_TTL: Duration = Duration::from_secs(6);

struct AppState {
    editor: Editor,
    bg_receiver: tokio::sync::mpsc::UnboundedReceiver<BackgroundEvent>,
    atlas: GpuGlyphAtlas,
    atlas_ppx: f32,
    cell_w: u32,
    cell_h: u32,
    surface: Surface,
    terminal_grid: (usize, u32, u32),
    surface_sig: u64,
    gutter_sig: u64,
    image_pane_key: u64,
    rendered_revision: u64,
    message: String,
    workspace: WorkspaceRuntime,
    tree_nodes: Vec<FileNode>,
    dirs: HashSet<String>,
    expanded: HashSet<String>,
    tree_epoch: u64,
    tree_root: Option<PathBuf>,
    rail: i32,
    sidebar_open: bool,
    /// Per-tool dock widths (project, commit, debug); the drag handle edits
    /// the active one.
    sidebar_widths: [f32; 3],
    /// DAP variablesReferences the user opened in the variables tree.
    debug_expanded: HashSet<i64>,
    /// The scope reference those expansions belong to; a new stop resets.
    debug_scope_seen: Option<i64>,
    /// Slot table from the last debug sync: UI row ids → DAP ids.
    debug_ref_slots: Vec<i64>,
    /// Renders wait for the window's real size once; the pre-map stub sizes
    /// would otherwise derive a viewport scroll that never unwinds.
    window_ready: bool,
    last_logical_width: f32,
    theme_name: String,
    app_font_name: String,
    app_font_size: f32,
    menu_title: Option<&'static str>,
    menu_x: f32,
    menu_visible: bool,
    bottom: i32,
    agent_open: bool,
    /// Pinned session for the agent dock; `None` follows the newest.
    agent_active: Option<u64>,
    /// Transcript entries the user folded open, keyed by (session, index).
    agent_expanded: HashSet<(u64, usize)>,
    agent_dock_width: f32,
    last_px: f32,
    last_py: f32,
    hovering: bool,
    dialog_visible: bool,
    dialog_title: String,
    dialog_input: String,
    settings_visible: bool,
    settings_category: i32,
    settings_search: String,
    /// Options/page changed and the settings rows need re-projection.
    settings_rows_dirty: bool,
    terminal_scroll: usize,
    git_history: Vec<GitHistoryRow>,
    wheel_y: f32,
    wheel_x: f32,
    welcome_visible: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    slint::BackendSelector::new().select()?;
    // Register the bundled Symbols Nerd Font so icon-pack glyphs render.
    {
        use slint::fontique_011::fontique;
        let bytes: &[u8] = include_bytes!("../../../assets/fonts/SymbolsNerdFont-Regular.ttf");
        let blob = fontique::Blob::new(std::sync::Arc::new(bytes.to_vec()));
        let mut collection = slint::fontique_011::shared_collection();
        let _ = collection.register_fonts(blob, None);
    }
    #[cfg(feature = "janet")]
    let rt = {
        let r = scripting::JanetRuntime::new();
        Some(Box::new(r) as Box<dyn kernel::script_runtime::ScriptRuntime>)
    };
    #[cfg(not(feature = "janet"))]
    let rt = None;

    let boot = boot::bootstrap(
        BootOptions {
            interactive: true,
            reopen_last_project: true,
        },
        rt,
    )?;

    let mut editor = boot.editor;
    if editor.font_config.loaded_fonts.is_empty() {
        editor.font_config.bootstrap_default_font();
    }
    if editor.buffers.is_empty() {
        let key = editor.create_buffer("*scratch*");
        if let Some(win) = editor.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
    }
    let mut atlas = GpuGlyphAtlas::new();
    if let Some(bytes) = editor.font_config.primary_font() {
        atlas.rasterize(bytes, editor.font_config.size);
    }
    let (init_cell_w, init_cell_h) = editor
        .font_config
        .primary_font()
        .map(|b| pane::measure_cell(b, editor.font_config.size))
        .unwrap_or((7, 17));

    let workspace = WorkspaceRuntime::new(&editor);
    let state = Rc::new(RefCell::new(AppState {
        editor,
        bg_receiver: boot.bg_receiver,
        atlas,
        atlas_ppx: 0.0,
        cell_w: init_cell_w,
        cell_h: init_cell_h,
        surface: Surface::new(1, 1),
        terminal_grid: (0, 0, 0),
        surface_sig: 0,
        gutter_sig: 0,
        image_pane_key: 0,
        rendered_revision: u64::MAX,
        message: String::new(),
        workspace,
        tree_nodes: Vec::new(),
        dirs: HashSet::new(),
        expanded: HashSet::new(),
        tree_epoch: u64::MAX,
        tree_root: None,
        rail: 0,
        sidebar_open: true,
        sidebar_widths: [240.0, 300.0, 420.0],
        debug_expanded: HashSet::new(),
        debug_scope_seen: None,
        debug_ref_slots: Vec::new(),
        window_ready: false,
        last_logical_width: 0.0,
        theme_name: String::new(),
        app_font_name: String::new(),
        app_font_size: 12.0,
        menu_title: None,
        menu_x: 0.0,
        menu_visible: false,
        bottom: -1,
        agent_open: false,
        agent_active: None,
        agent_expanded: HashSet::new(),
        agent_dock_width: 380.0,
        last_px: 0.0,
        last_py: 0.0,
        hovering: false,
        dialog_visible: false,
        dialog_title: String::new(),
        dialog_input: String::new(),
        settings_visible: false,
        settings_category: 0,
        settings_search: String::new(),
        settings_rows_dirty: false,
        terminal_scroll: 0,
        git_history: Vec::new(),
        wheel_y: 0.0,
        wheel_x: 0.0,
        welcome_visible: false,
    }));

    let app = AppWindow::new()?;

    actions::install(&app, Rc::clone(&state));

    sync::refresh(&app.as_weak(), &mut state.borrow_mut());

    let timer = slint::Timer::default();
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(20),
            move || {
                let Ok(mut s) = state.try_borrow_mut() else {
                    return;
                };
                let s = &mut *s;
                s.workspace.maintain(&mut s.editor);
                let mut changed = false;
                while let Ok(event) = s.bg_receiver.try_recv() {
                    process_background_event(&mut s.editor, event);
                    changed = true;
                }
                if changed {
                    s.editor.frame_dirty = true;
                }
                let resized = weak.upgrade().is_some_and(|app| {
                    // The window is unmapped during the first refresh, so its
                    // scale factor only becomes final once the backend shows it.
                    let ppx = app.window().scale_factor();
                    if (ppx - s.atlas_ppx).abs() > 0.001 {
                        return true;
                    }
                    let (cols, rows) = sync::editor_grid(&app, s.cell_w, s.cell_h);
                    s.surface.width != cols as u16 || s.surface.height != rows as u16
                });
                if resized
                    || s.editor.frame_dirty
                    || s.rendered_revision != sync::revision_of(&s.editor)
                {
                    s.editor.frame_dirty = false;
                    sync::refresh(&weak, s);
                }
            },
        );
    }
    app.run()?;
    drop(timer);
    {
        let mut s = state.borrow_mut();
        workspace::persist_session(&s.editor);
        workspace::close_editor_session(&mut s.editor);
    }
    drop(state);
    boot::shutdown(boot.runtime);
    Ok(())
}

fn logical_size(app: &AppWindow) -> (f32, f32) {
    let size = app.window().size();
    let scale = app.window().scale_factor().max(1.0);
    (size.width as f32 / scale, size.height as f32 / scale)
}
