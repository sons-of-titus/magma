//! The per-tick projection pass: editor state → Slint properties.
//!
//! `refresh` runs when the frame timer sees a dirty flag, a revision change,
//! or a resize.  Split across `agent` (agent dock + dialogs) and `overlays`
//! (hover/completion/quickfix/minimap) so each surface owns its sync.

mod agent;
mod db;
mod overlays;
mod render;
mod terminal;
mod tool_windows;
mod tree;

use std::time::Duration;

use slint::ComponentHandle;
use slint::{ModelRc, VecModel};

use kernel::render::frame::{render_frame_with, sync_viewport_scroll};

use crate::pane;
use crate::{AppState, AppWindow, TabItem, logical_size, projection, theme};
use kernel::render::CursorPaint;
use kernel::render::tabs;
use kernel::state::Editor;

const STATUS_TTL: Duration = Duration::from_secs(6);

/// The milestone log fires on the first stabilized layout only; welcome
/// transitions re-arm the gate and would otherwise log a second "first".
static FIRST_FRAME_LOGGED: std::sync::OnceLock<()> = std::sync::OnceLock::new();

/// Re-projects a model-valued property only when the row data actually
/// changed: replacing the model every refresh dirties Slint's scene graph
/// and forces a full re-layout even when nothing moved.
pub(crate) struct ModelGate<T> {
    last: Vec<T>,
}

impl<T: PartialEq + Clone + 'static> ModelGate<T> {
    pub(crate) fn new() -> Self {
        ModelGate { last: Vec::new() }
    }

    pub(crate) fn set(&mut self, next: Vec<T>, set: impl FnOnce(ModelRc<T>)) {
        if self.last == next {
            return;
        }
        self.last = next.clone();
        set(ModelRc::new(VecModel::from(next)));
    }
}

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
    // Window geometry is noisy before the first real layout (stubs of 0,
    // 480, then the true size within three frames).  Chrome keeps syncing —
    // the window must never show an empty half-drawn shell — but the editor
    // surface waits for a stable size: rendering from a stub would derive a
    // viewport scroll that never unwinds, and painting before the layout
    // lands is what makes the pane cover only part of the window.
    let mut viewport_ready = state.window_ready;
    if !viewport_ready {
        let (lw, _) = logical_size(&app);
        if lw < 200.0 || (lw - state.last_logical_width).abs() >= 1.0 {
            state.last_logical_width = lw;
        } else {
            state.window_ready = true;
            viewport_ready = true;
            if FIRST_FRAME_LOGGED.set(()).is_ok() {
                logging::info!(
                    "milestone: first useful frame at {:?}",
                    crate::boot_clock().elapsed()
                );
            }
        }
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
            let (inset, beam) = pane::caret_metrics_fallback(bytes, state.editor.font_config.size);
            state.caret_inset = inset;
            state.caret_height = beam;
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
        // The caret beam is the capital's ink span, measured from the
        // freshly rasterized pixels rather than trusted from font metrics,
        // which the device-scale rasterizer can quantize away from.
        if let Some((top, bottom)) = pane::atlas_ink_span_px(&state.atlas) {
            state.caret_inset = top as f32 / ppx;
            state.caret_height = (bottom - top + 1) as f32 / ppx;
        }
        // The grid did not change, but every glyph pixel did.
        state.surface_sig = 0;
        state.gutter_sig = 0;
    }

    sync_theme(&app, state);
    app.set_app_font_name(state.app_font_name.as_str().into());
    app.set_app_font_size(state.app_font_size);
    kernel::icons::sync_option(&mut state.editor);
    tree::sync_tree(&app, state);

    let ed = &mut state.editor;
    let (logical_w, _) = logical_size(&app);
    if viewport_ready {
        let (cols, rows) = editor_grid(&app, state.cell_w, state.cell_h);
        if state.surface.width != cols as u16 || state.surface.height != rows as u16 {
            state.surface.resize(cols as u16, rows as u16);
            ed.view_tree.resize(cols as u16, rows as u16);
            state.rendered_revision = u64::MAX;
            state.surface_sig = 0;
            state.gutter_sig = 0;
        }
        sync_viewport_scroll(ed);
        // Caret and completion popup are native overlays (a grid-stamped
        // caret can only color whole cells); the kernel draws neither.
        render_frame_with(ed, &mut state.surface, CursorPaint::Skip);
        render::sync_pane_images(&app, state, ppx);
    } else {
        // Hold the pane blank until the layout is honest — a surface sized
        // from stub geometry would paint only part of the view.
        app.set_pane(slint::Image::default());
        app.set_pane_width(0.0);
        app.set_pane_height(0.0);
        app.set_gutter_visible(false);
        app.set_gutter(slint::Image::default());
    }
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
                    // Decoding an SVG per refresh is pure waste; icon paths
                    // repeat every frame, so cache the decoded image.
                    let img = match state.icon_images.get(&p) {
                        Some(img) => img.clone(),
                        None => {
                            let img = slint::Image::load_from_path(std::path::Path::new(&p))
                                .unwrap_or_default();
                            state.icon_images.insert(p.clone(), img.clone());
                            img
                        }
                    };
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
    state.m_tabs.set(tab_models, |m| app.set_tabs(m));

    app.set_active_rail(state.rail);
    app.set_sidebar_open(state.sidebar_open);
    let rail = state.rail.clamp(0, 2) as usize;
    app.set_sidebar_width(state.sidebar_widths[rail]);

    let crumbs: Vec<slint::SharedString> = projection::breadcrumb_segments(ed)
        .into_iter()
        .map(Into::into)
        .collect();
    state.m_crumbs.set(crumbs, |m| app.set_breadcrumbs(m));

    let (file, modified) = projection::focused_file(ed);
    app.set_status_file(file.as_str().into());
    app.set_status_modified(modified);
    app.set_status_position(projection::cursor_position(ed).as_str().into());
    app.set_status_branch(projection::branch_label(ed).as_str().into());
    let (message, error) = projection::status_message(ed, &state.message, STATUS_TTL);
    app.set_status_message(message.as_str().into());
    app.set_status_error(error);
    // The async-open pipeline shows on the bottom progress strip; nothing
    // here invents progress the kernel is not reporting.
    state
        .m_progress
        .set(projection::progress_tasks(ed), |m| app.set_progress_tasks(m));
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
    state.m_find_rows.set(projection::find_rows(ed), |m| {
        app.set_find_rows(m);
    });
    let (output_title, output_text) = projection::output(ed);
    app.set_output_title(output_title.as_str().into());
    app.set_output_text(output_text.as_str().into());
    state.m_problems.set(projection::problem_rows(ed), |m| {
        app.set_problems(m);
    });
    app.set_dock_height(state.bottom_height);
    app.set_output_follow(state.output_follow);
    state.m_git_history.set(state.git_history.clone(), |m| {
        app.set_git_history(m);
    });
    app.set_status_mode(projection::status_mode(ed).as_str().into());
    state.m_recents.set(
        projection::recent_projects(ed)
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
        |m| app.set_recents(m),
    );
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
        // The window just changed size; re-arm the stabilization gate so the
        // pane renders from the new layout, never from the one being replaced.
        state.window_ready = false;
        state.last_logical_width = 0.0;
    }
    app.set_show_welcome(now_welcome);

    agent::sync(&app, state);
    tool_windows::sync_commit(&app, &mut state.editor, &state.message);
    tool_windows::sync_debug(&app, state);
    db::sync(&app, state);

    app.set_menu_visible(state.menu_visible);
    app.set_menu_x(state.menu_x);
    let items = match state.menu_title {
        Some(title) => projection::menu_items(title),
        None => Vec::new(),
    };
    state.m_menu.set(items, |m| app.set_menu_items(m));
    if viewport_ready {
        terminal::sync(&app, state, logical_w, ppx);
        overlays::sync(&app, state);
    }

    state.rendered_revision = revision_of(&state.editor);
    // Input callbacks refresh synchronously; without this the frame timer
    // would run the whole pipeline a second time ≤20 ms later.
    state.editor.frame_dirty = false;
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

