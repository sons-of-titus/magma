//! The per-tick projection pass: editor state → Slint properties.
//!
//! `refresh` runs when the frame timer sees a dirty flag, a revision change,
//! or a resize.  Split across `agent` (agent dock + dialogs) and `overlays`
//! (hover/completion/quickfix/minimap) so each surface owns its sync.

mod agent;
mod db;
mod overlays;
mod render;
mod tool_windows;

use std::time::Duration;

use slint::ComponentHandle;
use slint::{ModelRc, VecModel};

use kernel::render::frame::{render_frame_with, sync_viewport_scroll};
use std::collections::HashSet;

use crate::pane;
use crate::{AppState, AppWindow, TabItem, logical_size, projection, theme};
use kernel::project::tree::{build, collect_dirs};
use kernel::render::CursorPaint;
use kernel::render::tabs;
use kernel::state::Editor;

const STATUS_TTL: Duration = Duration::from_secs(6);

/// Editor grid from the actual laid-out pane, so the image, the surface, and
/// click hit-testing all agree with what is on screen.  Before the first
/// layout pass the element has no size yet; fall back to the window size
/// minus the fixed chrome (menu, tabs, breadcrumb, bottom bar, status).
pub(crate) fn editor_grid(app: &AppWindow, cell_w: u32, cell_h: u32) -> (u32, u32) {
    let mut w = app.get_editor_view_width() as f32;
    let mut h = app.get_editor_view_height() as f32;
    // Before the first real layout pass the view reports its minimum size.
    // Deriving the viewport scroll from that postage stamp sticks once the
    // pane reaches its real size — the cursor-follow rules only guarantee
    // the cursor stays inside, never that the view re-centers — so treat
    // unbelievable sizes the same as "no size yet".
    if w < 200.0 || h < 100.0 {
        let (mut lw, mut lh) = logical_size(app);
        // A not-yet-mapped window reports a stub size; sizing the surface
        // from it would freeze the viewport scroll at a postage stamp.
        // The window's preferred size is the honest guess until the real
        // layout lands.
        if lw < 300.0 || lh < 200.0 {
            lw = 1280.0;
            lh = 800.0;
        }
        w = (lw - 304.0).max(80.0);
        h = (lh - 132.0).max(40.0);
    }
    (
        (w / cell_w.max(1) as f32).floor().max(1.0) as u32,
        (h / cell_h.max(1) as f32).floor().max(1.0) as u32,
    )
}

pub(crate) fn revision_of(editor: &Editor) -> u64 {
    editor
        .buffers
        .iter()
        .map(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).revision())
        .sum()
}

pub(crate) fn refresh(weak: &slint::Weak<AppWindow>, state: &mut AppState) {
    let Some(app) = weak.upgrade() else { return };
    if !state.window_ready {
        // Window geometry is noisy before the first real layout (stubs of
        // 0, 480, then the true size within three frames).  Rendering from a
        // stub would derive a viewport scroll that never unwinds, so wait
        // until the size stops changing.
        let (lw, _) = logical_size(&app);
        if lw < 200.0 || (lw - state.last_logical_width).abs() >= 1.0 {
            state.last_logical_width = lw;
            return;
        }
        state.window_ready = true;
    }

    // `:vc` asks for the Version Control tool window; reveal it before the
    // chrome is synced so it appears in the same frame.
    if state.editor.vc.request_open {
        state.editor.vc.request_open = false;
        state.rail = 1;
    }
    // The Debug tool window exists only while a session does: when the last
    // session ends, put the rail back on the project tree.
    if state.rail == 2 && state.editor.debug.active_session().is_none() {
        state.rail = 0;
    }

    // Commands and chrome actions queue editor events (buffer-created drives
    // syntax highlighting and LSP attach); nothing else would drain them.
    state.editor.dispatch_events_scoped();

    let ppx = app.window().scale_factor() as f32;
    let ppx = if ppx < 1.0 { 1.0 } else { ppx };
    let font_changed = state.editor.font_changed;
    if font_changed {
        state.editor.font_changed = false;
    }
    // Recompute the cell grid from the (possibly new) font metrics so the
    // editor and terminal stay tight instead of inheriting a fixed cell.
    if font_changed || state.cell_w == 0 {
        if let Some(bytes) = state.editor.font_config.primary_font() {
            let (cw, ch) = pane::measure_cell(bytes, state.editor.font_config.size);
            state.cell_w = cw.max(1);
            state.cell_h = ch.max(1);
        }
    }
    let atlas_rebuilt = font_changed || (ppx - state.atlas_ppx).abs() > 0.001;
    if atlas_rebuilt {
        if font_changed {
            state.editor.row_layouts.clear();
        }
        if let Some(bytes) = state.editor.font_config.primary_font() {
            state.atlas.rasterize_for_grid(
                bytes,
                state.editor.font_config.size,
                ppx,
                state.cell_w,
                state.cell_h,
            );
        }
        state.atlas_ppx = ppx;
        // The grid did not change, but every glyph pixel did.
        state.surface_sig = 0;
        state.gutter_sig = 0;
    }

    sync_theme(&app, state);
    app.set_app_font_name(state.app_font_name.as_str().into());
    app.set_app_font_size(state.app_font_size);
    kernel::icons::sync_option(&mut state.editor);
    sync_tree(&app, state);

    let ed = &mut state.editor;
    let (logical_w, _) = logical_size(&app);
    let (cols, rows) = editor_grid(&app, state.cell_w, state.cell_h);
    if state.surface.width != cols as u16 || state.surface.height != rows as u16 {
        state.surface.resize(cols as u16, rows as u16);
        ed.view_tree.resize(cols as u16, rows as u16);
        state.rendered_revision = u64::MAX;
        state.surface_sig = 0;
        state.gutter_sig = 0;
    }
    sync_viewport_scroll(ed);
    render_frame_with(ed, &mut state.surface, CursorPaint::Stamp { visible: true });
    render::sync_pane_images(&app, state, ppx);
    let ed = &mut state.editor;
    let focused = ed.view_tree.focused_window();
    let tab_models: Vec<TabItem> = tabs::collect_tabs(ed)
        .into_iter()
        .map(|t| {
            let (icon, icon_color, icon_path, icon_image) = match t.icon {
                Some(kernel::icons::IconKind::Glyph { glyph, color }) => (
                    glyph.to_string(),
                    color
                        .map(|(r, g, b)| slint::Color::from_rgb_u8(r, g, b))
                        .unwrap_or(slint::Color::from_rgb_u8(166, 173, 200)),
                    String::new(),
                    slint::Image::default(),
                ),
                Some(kernel::icons::IconKind::Svg { path }) => {
                    let p = path.to_string_lossy().into_owned();
                    let img =
                        slint::Image::load_from_path(std::path::Path::new(&p)).unwrap_or_default();
                    (
                        String::new(),
                        slint::Color::from_rgb_u8(166, 173, 200),
                        p,
                        img,
                    )
                }
                None => (
                    String::new(),
                    slint::Color::from_rgb_u8(166, 173, 200),
                    String::new(),
                    slint::Image::default(),
                ),
            };
            TabItem {
                title: t.label.into(),
                active: focused == Some(t.window_id),
                modified: t.modified,
                icon: icon.into(),
                icon_color,
                icon_path: icon_path.into(),
                icon_image,
            }
        })
        .collect();
    app.set_tabs(ModelRc::new(VecModel::from(tab_models)));

    app.set_active_rail(state.rail);
    app.set_sidebar_open(state.sidebar_open);
    let rail = state.rail.clamp(0, 2) as usize;
    app.set_sidebar_width(state.sidebar_widths[rail]);

    let crumbs: Vec<slint::SharedString> = projection::breadcrumb_segments(ed)
        .into_iter()
        .map(Into::into)
        .collect();
    app.set_breadcrumbs(ModelRc::new(VecModel::from(crumbs)));

    let (file, modified) = projection::focused_file(ed);
    app.set_status_file(file.as_str().into());
    app.set_status_modified(modified);
    app.set_status_position(projection::cursor_position(ed).as_str().into());
    app.set_status_branch(projection::branch_label(ed).as_str().into());
    let (message, error) = projection::status_message(ed, &state.message, STATUS_TTL);
    app.set_status_message(message.as_str().into());
    app.set_status_error(error);
    app.set_pane_message(projection::pane_message(ed).as_str().into());

    app.set_bottom_tool(state.bottom);
    app.set_bottom_problems_count(ed.quickfix_list.len() as i32);
    app.set_bottom_terminal_count(ed.io.terminals.sessions.len() as i32);
    app.set_bottom_title(
        match state.bottom {
            0 => "Terminal",
            1 => "Find in Files",
            2 => "Output",
            3 => "Git History",
            4 => "Problems",
            _ => "",
        }
        .into(),
    );
    app.set_terminal_text(projection::terminal_text(ed).into());
    // terminal sessions
    {
        let sessions: Vec<slint::SharedString> = ed
            .io
            .terminals
            .sessions
            .iter()
            .map(|s| s.title.as_str().into())
            .collect();
        app.set_terminal_sessions(slint::ModelRc::new(slint::VecModel::from(sessions)));
        let active = ed
            .io
            .terminals
            .focused
            .and_then(|fid| ed.io.terminals.sessions.iter().position(|s| s.id == fid))
            .unwrap_or(0) as i32;
        app.set_terminal_active(active);
    }
    // terminal image (styled, device resolution)
    {
        let session_count = ed.io.terminals.sessions.len();
        let (term_cols, term_rows) =
            pane::terminal_grid_size(logical_w, session_count, state.cell_w, state.cell_h);
        let term_id = ed
            .io
            .terminals
            .focused
            .or_else(|| ed.io.terminals.sessions.first().map(|s| s.id));
        if state.bottom == 0
            && let Some(id) = term_id
        {
            if state.terminal_grid != (id, term_cols, term_rows) {
                if let Err(e) = ed
                    .io
                    .terminals
                    .resize(id, term_cols as u16, term_rows as u16)
                {
                    state.message = e;
                }
                state.terminal_grid = (id, term_cols, term_rows);
            }
            if let Some(sess) = ed.io.terminals.get(id) {
                let total = sess.screen.line_count();
                let visible = term_rows as usize;
                let max_scroll = total.saturating_sub(visible);
                if state.terminal_scroll > max_scroll {
                    state.terminal_scroll = max_scroll;
                }
                let start = total
                    .saturating_sub(visible)
                    .saturating_sub(state.terminal_scroll);
                let count = visible.min(total - start);
                let lines: Vec<_> = (start..start + count)
                    .map(|i| sess.screen.styled_line(i))
                    .collect();
                let cursor = if ed.io.terminals.focused == Some(id) && sess.screen.cursor_visible()
                {
                    let line =
                        sess.screen.scrollback_len() + sess.screen.cursor_screen_row() as usize;
                    line.checked_sub(start)
                        .filter(|row| *row < count)
                        .map(|row| (row, sess.screen.cursor_col() as usize))
                } else {
                    None
                };
                let terminal_palette = theme::palette(&state.theme_name).terminal;
                let (cell_w, cell_h) = pane::cell_size(state.cell_w, state.cell_h, ppx);
                let (rgba, tw, th) = pane::blit_terminal_rgba(
                    &lines,
                    &state.atlas,
                    term_cols,
                    term_rows,
                    cell_w,
                    cell_h,
                    &terminal_palette,
                    cursor,
                );
                let mut buffer =
                    slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(tw.max(1), th.max(1));
                let dst = buffer.make_mut_bytes();
                let n = dst.len().min(rgba.len());
                dst[..n].copy_from_slice(&rgba[..n]);
                app.set_terminal_image(slint::Image::from_rgba8(buffer));
            }
        } else if state.bottom == 0 {
            app.set_terminal_image(slint::Image::default());
        }
    }
    app.set_find_rows(ModelRc::new(VecModel::from(projection::find_rows(ed))));
    let (output_title, output_text) = projection::output(ed);
    app.set_output_title(output_title.as_str().into());
    app.set_output_text(output_text.as_str().into());
    app.set_problems(slint::ModelRc::new(slint::VecModel::from(
        projection::problem_rows(ed),
    )));
    app.set_dock_height(state.bottom_height);
    app.set_output_follow(state.output_follow);
    app.set_git_history(ModelRc::new(VecModel::from(state.git_history.clone())));
    app.set_status_mode(projection::status_mode(ed).as_str().into());
    app.set_recents(ModelRc::new(VecModel::from(
        projection::recent_projects(ed)
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));
    let now_welcome = ed.project_manager.project.root.is_none();
    if now_welcome != state.welcome_visible {
        state.welcome_visible = now_welcome;
        if now_welcome {
            app.window()
                .set_size(slint::WindowSize::Logical(slint::LogicalSize::new(
                    640.0, 480.0,
                )));
        } else {
            let (w, _) = logical_size(&app);
            if w < 900.0 {
                app.window()
                    .set_size(slint::WindowSize::Logical(slint::LogicalSize::new(
                        1280.0, 800.0,
                    )));
            }
        }
    }
    app.set_show_welcome(now_welcome);

    agent::sync(&app, state);
    tool_windows::sync_commit(&app, &mut state.editor, &state.message);
    tool_windows::sync_debug(&app, state);
    db::sync(&app, state);

    app.set_menu_visible(state.menu_visible);
    app.set_menu_x(state.menu_x);
    let items = state
        .menu_title
        .map(projection::menu_items)
        .map(|items| ModelRc::new(VecModel::from(items)))
        .unwrap_or_else(|| ModelRc::new(VecModel::from(Vec::new())));
    app.set_menu_items(items);
    overlays::sync(&app, state);

    state.rendered_revision = revision_of(&state.editor);
}

/// Commit tool window: repository identity, grouped changes, busy and error
/// surfaces — all read from the live VC state, never from UI-local mirrors.
fn sync_theme(app: &AppWindow, state: &mut AppState) {
    let name = state
        .editor
        .options
        .get("gui-theme")
        .cloned()
        .unwrap_or_else(|| "intellij-dark".into());
    if state.theme_name == name {
        return;
    }
    let palette = theme::palette(&name);
    theme::apply_slint(app, &palette, &state.editor.font_config.family);
    theme::apply_editor(&mut state.editor, &palette);
    state.theme_name = name;
}

fn sync_tree(app: &AppWindow, state: &mut AppState) {
    let project = &state.editor.project_manager.project;
    let root = project.root.clone();
    if state.tree_epoch != project.files_epoch || state.tree_root != root {
        state.tree_epoch = project.files_epoch;
        state.tree_root = root;
        let mut all: Vec<String> = project.files.clone();
        all.extend(project.ignored_files.iter().cloned());
        let ignored: HashSet<String> = project.ignored_files.clone();
        state.tree_nodes = build(&all, &ignored);
        state.dirs.clear();
        collect_dirs(&state.tree_nodes, &mut state.dirs);
        state.expanded.clear();
        for node in &state.tree_nodes {
            if node.is_dir {
                state.expanded.insert(node.path.clone());
            }
        }
    }

    let (title, empty) = match state.rail {
        1 => ("Commit", ""),
        2 => ("Debugger", ""),
        _ => (
            "Project",
            if state.tree_nodes.is_empty() {
                "No project open. Use File › Open Folder…"
            } else {
                "No matching files"
            },
        ),
    };
    app.set_tree_title(title.into());
    app.set_tree_empty(empty.into());

    let rows = if state.rail == 0 {
        projection::tree_rows(&state.editor, &state.tree_nodes, &state.expanded)
    } else {
        Vec::new()
    };
    app.set_tree(ModelRc::new(VecModel::from(rows)));
    app.set_selected_path(projection::current_path(&state.editor).into());
}
