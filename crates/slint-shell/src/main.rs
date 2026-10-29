//! Slint frontend — a declarative shell over the kernel's `Surface` pipeline.
//!
//! The UI is split into reusable components (`ui/components/`) over a single
//! `Theme` global.  This file owns the event loop and the action boundary:
//! components emit commands, the shell applies them through the same registry
//! every other frontend uses.

mod actions;
mod catppuccin;
mod dracula;
mod input;
mod lifecycle;
mod new_palettes;
mod noctis;
mod palettes;
mod pane;
mod projection;
mod sync;
mod theme;
mod tokyo_night;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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

struct AppState {
    editor: Editor,
    bg_receiver: tokio::sync::mpsc::UnboundedReceiver<BackgroundEvent>,
    atlas: GpuGlyphAtlas,
    atlas_ppx: f32,
    cell_w: u32,
    cell_h: u32,
    /// Empty leading above the glyphs in a row; the caret starts below it.
    caret_inset: f32,
    /// Beam height in logical px: cap line to baseline, measured from the
    /// atlas like `caret_inset` so the beam ends where the ink ends.
    caret_height: f32,
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
    /// Bottom dock height; the top-edge drag handle edits it.
    bottom_height: f32,
    /// Output pane follows the newest lines as they land.
    output_follow: bool,
    /// Per-tool dock widths (project, commit, debug); the drag handle edits
    /// the active one.
    sidebar_widths: [f32; 3],
    /// DAP variablesReferences the user opened in the variables tree.
    /// The Database tool window on the right, beneath the Agent dock.
    db_open: bool,
    db_height: f32,
    db_form_open: bool,
    db_form_note: String,
    db_expanded: HashSet<usize>,
    debug_expanded: HashSet<i64>,
    /// The scope reference those expansions belong to; a new stop resets.
    debug_scope_seen: Option<i64>,
    /// Slot table from the last debug sync: UI row ids → DAP ids.
    debug_ref_slots: Vec<i64>,
    /// Chrome syncs from the first frame; only the editor surface waits for
    /// the window's real size — the pre-map stub sizes would otherwise
    /// derive a viewport scroll that never unwinds.
    window_ready: bool,
    last_logical_width: f32,
    /// The launcher window; visible exactly while no project is open.
    welcome: slint::Weak<WelcomeWindow>,
    launcher_visible: bool,
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
    /// The unsaved-changes dialog raised over a refused quit.
    quit_confirm_visible: bool,
    /// Save was chosen in that dialog; quit once the dirty set drains
    /// (async writes keep buffers dirty until the write-confirmed events
    /// land).
    quit_when_clean: bool,
    settings_visible: bool,
    settings_category: i32,
    settings_search: String,
    /// Options/page changed and the settings rows need re-projection.
    settings_rows_dirty: bool,
    terminal_scroll: usize,
    git_history: Vec<GitHistoryRow>,
    wheel_y: f32,
    wheel_x: f32,
    /// Signature gates so idle refreshes skip re-blitting textures.
    minimap_sig: u64,
    terminal_sig: u64,
    /// Decoded file-type icons, keyed by SVG path.
    icon_images: HashMap<String, slint::Image>,
    m_tabs: sync::ModelGate<TabItem>,
    m_tree: sync::ModelGate<TreeItem>,
    m_crumbs: sync::ModelGate<slint::SharedString>,
    m_find_rows: sync::ModelGate<FindRow>,
    m_problems: sync::ModelGate<ProblemRow>,
    m_progress: sync::ModelGate<ProgressTask>,
    m_recents: sync::ModelGate<slint::SharedString>,
    m_sessions: sync::ModelGate<slint::SharedString>,
    m_git_history: sync::ModelGate<GitHistoryRow>,
    m_menu: sync::ModelGate<MenuItem>,
    m_hover_lines: sync::ModelGate<slint::SharedString>,
    m_completion_items: sync::ModelGate<slint::SharedString>,
    m_quickfix_items: sync::ModelGate<slint::SharedString>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = boot_clock();
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
    logging::info!(
        "milestone: editor bootstrapped at {:?}",
        boot_clock().elapsed()
    );

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
    let (init_cell_w, init_cell_h, init_caret_inset, init_caret_height) = editor
        .font_config
        .primary_font()
        .map(|b| {
            let (w, h) = pane::measure_cell(b, editor.font_config.size);
            let (inset, beam) = pane::caret_metrics_fallback(b, editor.font_config.size);
            (w, h, inset, beam)
        })
        .unwrap_or((7, 17, 0.0, 17.0));

    let workspace = WorkspaceRuntime::new(&editor);
    let state = Rc::new(RefCell::new(AppState {
        editor,
        bg_receiver: boot.bg_receiver,
        atlas,
        atlas_ppx: 0.0,
        cell_w: init_cell_w,
        cell_h: init_cell_h,
        caret_inset: init_caret_inset,
        caret_height: init_caret_height,
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
        bottom_height: 240.0,
        output_follow: true,
        sidebar_widths: [240.0, 300.0, 420.0],
        db_open: false,
        db_height: 340.0,
        db_form_open: false,
        db_form_note: String::new(),
        db_expanded: HashSet::new(),
        debug_expanded: HashSet::new(),
        debug_scope_seen: None,
        debug_ref_slots: Vec::new(),
        window_ready: false,
        last_logical_width: 0.0,
        welcome: slint::Weak::default(),
        launcher_visible: false,
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
        quit_confirm_visible: false,
        quit_when_clean: false,
        settings_visible: false,
        settings_category: 0,
        settings_search: String::new(),
        settings_rows_dirty: false,
        terminal_scroll: 0,
        git_history: Vec::new(),
        wheel_y: 0.0,
        wheel_x: 0.0,
        minimap_sig: 0,
        terminal_sig: 0,
        icon_images: HashMap::new(),
        m_tabs: sync::ModelGate::new(),
        m_tree: sync::ModelGate::new(),
        m_crumbs: sync::ModelGate::new(),
        m_find_rows: sync::ModelGate::new(),
        m_problems: sync::ModelGate::new(),
        m_progress: sync::ModelGate::new(),
        m_recents: sync::ModelGate::new(),
        m_sessions: sync::ModelGate::new(),
        m_git_history: sync::ModelGate::new(),
        m_menu: sync::ModelGate::new(),
        m_hover_lines: sync::ModelGate::new(),
        m_completion_items: sync::ModelGate::new(),
        m_quickfix_items: sync::ModelGate::new(),
    }));

    let app = AppWindow::new()?;
    let welcome = WelcomeWindow::new()?;
    logging::info!(
        "milestone: window created at {:?}",
        boot_clock().elapsed()
    );

    {
        let mut s = state.borrow_mut();
        s.welcome = welcome.as_weak();
    }

    actions::install(&app, Rc::clone(&state));
    lifecycle::install(&app, Rc::clone(&state));

    // The launcher routes into the same actions as the editor menus, then
    // refreshes the editor projection so the new project's first frame is
    // already projected when its window maps.
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        welcome.on_open_folder(move || {
            let mut s = state.borrow_mut();
            actions::open_project_dialog(&mut s);
            s.editor.frame_dirty = true;
            sync::refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        welcome.on_open_recent(move |path| {
            let mut s = state.borrow_mut();
            actions::open_project_path(&mut s, &path);
            s.editor.frame_dirty = true;
            sync::refresh(&weak, &mut s);
        });
    }

    // Persisted font choices go on before the first refresh so the first
    // frame already renders with them.
    {
        let mut s = state.borrow_mut();
        actions::fonts::restore_persisted_fonts(&app.as_weak(), &mut s);
    }

    // The project opens only after the window exists.  `open-project` now
    // establishes context synchronously and pushes the walk, session
    // restore, and task detection onto the pool, so nothing here can stall
    // the first frame.
    if let Some(path) = boot.reopen_project {
        let mut args = HashMap::new();
        args.insert(
            "path".to_string(),
            kernel::command::args::ArgValue::Path(path),
        );
        if let Err(e) =
            kernel::command::execute_command(&mut state.borrow_mut().editor, "open-project", &args)
        {
            logging::warn!("Could not reopen last project: {e}");
        }
    }

    // One of the two windows is always up: the launcher while no project is
    // open, the editor otherwise.  Opening a project maps the editor window
    // at full size and closes the launcher — no window is ever resized, so
    // an open never flashes a partial-size app.
    let launcher = {
        let mut s = state.borrow_mut();
        s.launcher_visible = s.editor.project_manager.project.root.is_none();
        s.launcher_visible
    };
    if launcher {
        let _ = welcome.show();
    } else {
        let _ = app.show();
    }

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
                // `quit`/`force-quit`/`save-and-quit` only set the flag — the
                // persist that the window-close path gets must happen here too.
                if !s.editor.running {
                    workspace::persist_session(&s.editor);
                    workspace::close_editor_session(&mut s.editor);
                    let _ = slint::quit_event_loop();
                    return;
                }
                // A refused `quit` (dirty buffers) raises the confirmation
                // dialog instead of dying on the status-bar error alone.
                if s.editor.quit_refused {
                    s.editor.quit_refused = false;
                    if !s.editor.dirty_buffer_names().is_empty() {
                        lifecycle::show_confirm(&weak, s);
                        return;
                    }
                }
                // "Save All" in the dialog: quit once the async writes land.
                if s.quit_when_clean && s.editor.dirty_buffer_names().is_empty() {
                    s.quit_when_clean = false;
                    s.quit_confirm_visible = false;
                    let _ = kernel::command::execute_command(
                        &mut s.editor,
                        "quit",
                        &HashMap::new(),
                    );
                }
                s.workspace.maintain(&mut s.editor);
                let mut changed = false;
                // Budgeted drain: a burst of loaded session buffers or walk
                // batches must not turn one tick into a long callback — the
                // rest continue next tick.
                let drain_deadline = std::time::Instant::now() + Duration::from_millis(4);
                while let Ok(event) = s.bg_receiver.try_recv() {
                    process_background_event(&mut s.editor, event);
                    changed = true;
                    if std::time::Instant::now() >= drain_deadline {
                        break;
                    }
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
    slint::run_event_loop()?;
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

/// Process-start anchor for startup-milestone timing logs.
fn boot_clock() -> std::time::Instant {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *START.get_or_init(std::time::Instant::now)
}

fn logical_size(app: &AppWindow) -> (f32, f32) {
    let size = app.window().size();
    let scale = app.window().scale_factor().max(1.0);
    (size.width as f32 / scale, size.height as f32 / scale)
}
