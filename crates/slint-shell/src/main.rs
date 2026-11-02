//! Slint frontend — a declarative shell over the kernel's `Surface` pipeline.
//!
//! The UI is split into reusable components (`ui/components/`) over a single
//! `Theme` global.  This file owns the event loop and the action boundary:
//! components emit commands, the shell applies them through the same registry
//! every other frontend uses.

mod actions;
mod app_state;
mod context;
mod catppuccin;
mod dracula;
mod input;
mod lifecycle;
mod new_palettes;
mod new_project;
mod noctis;
mod palettes;
mod pane;
mod projection;
mod sync;
mod theme;
mod tokyo_night;
mod window_chrome;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use kernel::boot::{self, BootOptions};
use kernel::render::gpu_atlas::GpuGlyphAtlas;
use kernel::render::surface::Surface;
use kernel::runtime::process_background_event;
use kernel::workspace::{self, WorkspaceRuntime};

use crate::actions::picker;
pub(crate) use crate::app_state::{system_prefers_dark, AppState};
#[cfg(feature = "janet")]
use scripting;
slint::include_modules!();

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
    // Follow the OS appearance for the first launch; a persisted choice
    // (Settings › Theme) always wins over the system default.
    if !editor.options.contains_key("gui-theme") {
        let dark = system_prefers_dark();
        editor.options.insert(
            "gui-theme".into(),
            if dark { "intellij-dark".into() } else { "intellij-light".into() },
        );
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
        output_cache: None,
        problems_cache: None,
        find_rows_cache: None,
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
        picker: picker::PickerKind::None,
        picker_query: String::new(),
        picker_selected: 0,
        picker_items: Vec::new(),
        visible_items: Vec::new(),
        find_visible: false,
        find_replace_visible: false,
        find_regex: false,
        find_query: String::new(),
        find_replacement: String::new(),
        find_match_text: String::new(),
        find_has_error: false,
        context_menu: None,
        context_path: String::new(),
        quit_confirm_visible: false,
        new_project: new_project::NewProjectState::default(),
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
        m_output_lines: sync::ModelGate::new(),
        m_problems: sync::ModelGate::new(),
        m_progress: sync::ModelGate::new(),
        m_recents: sync::ModelGate::new(),
        m_sessions: sync::ModelGate::new(),
        m_git_history: sync::ModelGate::new(),
        m_menu: sync::ModelGate::new(),
        m_new_project_templates: sync::ModelGate::new(),
        m_new_project_vars: sync::ModelGate::new(),
        m_new_project_languages: sync::ModelGate::new(),
        m_hover_lines: sync::ModelGate::new(),
        m_completion_entries: sync::ModelGate::new(),
        m_quickfix_items: sync::ModelGate::new(),
    }));

    let app = AppWindow::new()?;
    let welcome = WelcomeWindow::new()?;
    logging::info!("milestone: window created at {:?}", boot_clock().elapsed());

    {
        let mut s = state.borrow_mut();
        s.welcome = welcome.as_weak();
    }

    actions::install(&app, Rc::clone(&state));
    lifecycle::install(&app, Rc::clone(&state));
    window_chrome::install(&app);

    // The launcher routes into the same actions as the editor menus, then
    // refreshes the editor projection so the new project's first frame is
    // already projected when its window maps.
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        welcome.on_new_project(move || {
            let mut s = state.borrow_mut();
            new_project::open(&weak, &mut s);
            s.editor.frame_dirty = true;
            sync::refresh(&weak, &mut s);
        });
    }
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
                    let _ =
                        kernel::command::execute_command(&mut s.editor, "quit", &HashMap::new());
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
