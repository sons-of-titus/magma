//! Slint frontend — a declarative shell over the kernel's `Surface` pipeline.
//!
//! The UI is split into reusable components (`ui/components/`) over a single
//! `Theme` global.  This file owns the event loop and the action boundary:
//! components emit commands, the shell applies them through the same registry
//! every other frontend uses.

mod actions;
mod input;
mod pane;
mod projection;
mod theme;

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use magma::kernel::boot::{self, BootOptions};
#[cfg(feature = "janet")]
use scripting;
use magma::kernel::project::tree::{build, collect_dirs, FileNode};
use magma::kernel::render::frame::render_frame_with;
use magma::kernel::render::gpu_atlas::GpuGlyphAtlas;
use magma::kernel::render::surface::Surface;
use magma::kernel::render::tabs;
use magma::kernel::render::CursorPaint;
use magma::kernel::runtime::{process_background_event, BackgroundEvent};
use magma::kernel::state::Editor;
use magma::kernel::workspace::{self, WorkspaceRuntime};
use slint::{ModelRc, VecModel};

slint::include_modules!();

const STATUS_TTL: Duration = Duration::from_secs(6);

#[derive(Default)]
struct PaneTexState {
    device: Option<Arc<slint::wgpu_30::wgpu::Device>>,
    queue:  Option<Arc<slint::wgpu_30::wgpu::Queue>>,
    texture: Option<slint::wgpu_30::wgpu::Texture>,
    tex_w: u32,
    tex_h: u32,
    rgba: Vec<u8>,
    w: u32,
    h: u32,
    dirty: bool,
}

struct AppState {
    editor:            Editor,
    bg_receiver:       tokio::sync::mpsc::UnboundedReceiver<BackgroundEvent>,
    atlas:             GpuGlyphAtlas,
    surface:           Surface,
    tex_state:         Arc<Mutex<PaneTexState>>,
    rendered_revision: u64,
    message:           String,
    workspace:         WorkspaceRuntime,
    tree_nodes:        Vec<FileNode>,
    dirs:              HashSet<String>,
    expanded:          HashSet<String>,
    tree_epoch:        u64,
    tree_root:         Option<PathBuf>,
    rail:              i32,
    theme_name:        String,
    menu_title:        Option<&'static str>,
    menu_x:            f32,
    menu_visible:      bool,
    bottom:            i32,
    agent_open:        bool,
    last_px:           f32,
    last_py:           f32,
    hovering:          bool,
    dialog_visible:    bool,
    dialog_title:      String,
    dialog_input:      String,
    settings_visible:  bool,
    terminal_scroll:   usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    slint::BackendSelector::new()
        .require_wgpu_30(slint::wgpu_30::WGPUConfiguration::default())
        .select()?;
    #[cfg(feature = "janet")]
    let rt = {
        let r = scripting::JanetRuntime::new();
        Some(Box::new(r) as Box<dyn magma::kernel::script_runtime::ScriptRuntime>)
    };
    #[cfg(not(feature = "janet"))]
    let rt = None;

    let boot = boot::bootstrap(BootOptions {
        interactive:         true,
        reopen_last_project: true,
    }, rt)?;

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

    let workspace = WorkspaceRuntime::new(&editor);
    let tex_state = Arc::new(Mutex::new(PaneTexState::default()));
    let state = Rc::new(RefCell::new(AppState {
        editor,
        bg_receiver: boot.bg_receiver,
        atlas,
        surface: Surface::new(1, 1),
        tex_state: tex_state.clone(),
        rendered_revision: u64::MAX,
        message: String::new(),
        workspace,
        tree_nodes: Vec::new(),
        dirs: HashSet::new(),
        expanded: HashSet::new(),
        tree_epoch: u64::MAX,
        tree_root: None,
        rail: 0,
        theme_name: String::new(),
        menu_title: None,
        menu_x: 0.0,
        menu_visible: false,
        bottom:            -1,
        agent_open:        false,
        last_px:           0.0,
        last_py:           0.0,
        hovering:          false,
        dialog_visible:    false,
        dialog_title:      String::new(),
        dialog_input:      String::new(),
        settings_visible:  false,
        terminal_scroll:   0,
    }));

    let app = AppWindow::new()?;

    // ── GPU pane texture via rendering notifier ────────────────────────────
    {
        let tex_state = tex_state.clone();
        let weak = app.as_weak();
        app.window().set_rendering_notifier(move |state, graphics_api| {
            let (device, queue) = match graphics_api {
                slint::GraphicsAPI::WGPU30 { device, queue, .. } => (device.clone(), queue.clone()),
                _ => return,
            };
            match state {
                slint::RenderingState::RenderingSetup => {
                    let mut ts = tex_state.lock().unwrap();
                    ts.device = Some(Arc::new(device));
                    ts.queue = Some(Arc::new(queue));
                }
                slint::RenderingState::BeforeRendering => {
                    let mut ts = tex_state.lock().unwrap();
                    let device = match &ts.device { Some(d) => d.clone(), None => return };
                    let queue = match &ts.queue { Some(q) => q.clone(), None => return };
                    if !ts.dirty || ts.rgba.is_empty() { return; }
                    let w = ts.w;
                    let h = ts.h;
                    if ts.texture.is_none() || ts.tex_w != w || ts.tex_h != h {
                        ts.texture = Some(device.create_texture(&slint::wgpu_30::wgpu::TextureDescriptor {
                            label: Some("pane"),
                            size: slint::wgpu_30::wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: slint::wgpu_30::wgpu::TextureDimension::D2,
                            format: slint::wgpu_30::wgpu::TextureFormat::Rgba8Unorm,
                            usage: slint::wgpu_30::wgpu::TextureUsages::TEXTURE_BINDING | slint::wgpu_30::wgpu::TextureUsages::COPY_DST | slint::wgpu_30::wgpu::TextureUsages::RENDER_ATTACHMENT,
                            view_formats: &[],
                        }));
                        ts.tex_w = w;
                        ts.tex_h = h;
                    }
                    if let Some(tex) = &ts.texture {
                        queue.write_texture(
                            tex.as_image_copy(),
                            &ts.rgba,
                            slint::wgpu_30::wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(w * 4),
                                rows_per_image: None,
                            },
                            slint::wgpu_30::wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                        );
                        if let Ok(image) = slint::Image::try_from(tex.clone()) {
                            ts.dirty = false;
                            drop(ts);
                            if let Some(app) = weak.upgrade() {
                                app.set_pane(image);
                            }
                        }
                    }
                }
                _ => {}
            }
        }).unwrap();
    }

    actions::install(&app, Rc::clone(&state));

    refresh(&app.as_weak(), &mut state.borrow_mut());

    let timer = slint::Timer::default();
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        timer.start(slint::TimerMode::Repeated, Duration::from_millis(40), move || {
            let Ok(mut s) = state.try_borrow_mut() else { return; };
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
                let (w, h) = logical_size(&app);
                let (cols, rows) = pane::grid_size(w, h);
                s.surface.width != cols as u16 || s.surface.height != rows as u16
            });
            if resized || s.editor.frame_dirty || s.rendered_revision != revision_of(&s.editor) {
                s.editor.frame_dirty = false;
                refresh(&weak, s);
            }
        });
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

fn revision_of(editor: &Editor) -> u64 {
    editor
        .buffers
        .iter()
        .map(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).revision())
        .sum()
}

fn refresh(weak: &slint::Weak<AppWindow>, state: &mut AppState) {
    let Some(app) = weak.upgrade() else { return };

    sync_theme(&app, state);
    sync_tree(&app, state);

    let ed = &mut state.editor;
    let (logical_w, logical_h) = logical_size(&app);
    let (cols, rows) = pane::grid_size(logical_w, logical_h);
    if state.surface.width != cols as u16 || state.surface.height != rows as u16 {
        state.surface.resize(cols as u16, rows as u16);
        state.rendered_revision = u64::MAX;
    }
    render_frame_with(ed, &mut state.surface, CursorPaint::Stamp { visible: true });
    let (rgba, pw, ph) = pane::blit_rgba(&state.surface, &state.atlas);
    {
        let mut ts = state.tex_state.lock().unwrap();
        ts.rgba = rgba;
        ts.w = pw;
        ts.h = ph;
        ts.dirty = true;
    }
    app.set_pane_width((cols * pane::CHAR_W) as f32);
    app.set_pane_height((rows * pane::LINE_H) as f32);

    let focused = ed.view_tree.focused_window();
    let tab_models: Vec<TabItem> = tabs::collect_tabs(ed)
        .into_iter()
        .map(|t| TabItem {
            title: t.label.into(),
            active: focused == Some(t.window_id),
            modified: t.modified,
        })
        .collect();
    app.set_tabs(ModelRc::new(VecModel::from(tab_models)));

    app.set_active_rail(state.rail);

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
    app.set_bottom_title(match state.bottom {
        0 => "Terminal",
        1 => "Find in Files",
        2 => "Output",
        _ => "",
    }.into());
    app.set_terminal_text(projection::terminal_text(ed).into());
    // terminal sessions
    {
        let sessions: Vec<slint::SharedString> = ed.io.terminals.sessions.iter().map(|s| s.title.as_str().into()).collect();
        app.set_terminal_sessions(slint::ModelRc::new(slint::VecModel::from(sessions)));
        let active = ed.io.terminals.focused
            .and_then(|fid| ed.io.terminals.sessions.iter().position(|s| s.id == fid))
            .unwrap_or(0) as i32;
        app.set_terminal_active(active);
    }
    // terminal image (styled)
    {
        let session = ed.io.terminals.focused
            .or_else(|| ed.io.terminals.sessions.first().map(|s| s.id))
            .and_then(|id| ed.io.terminals.get(id));
        if let Some(sess) = session {
            let total = sess.screen.line_count();
            let visible = 200 / 16;
            let start = total.saturating_sub(visible).saturating_sub(state.terminal_scroll);
            let lines: Vec<_> = (start..start + visible.min(total - start)).map(|i| sess.screen.styled_line(i)).collect();
            let img = pane::blit_terminal(&lines, &state.atlas, 800, 200);
            app.set_terminal_image(img);
        } else {
            // empty image
            app.set_terminal_image(slint::Image::from_rgba8(slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(1,1)));
        }
    }
    app.set_find_rows(ModelRc::new(VecModel::from(projection::find_rows(ed))));
    let (output_title, output_text) = projection::output(ed);
    app.set_output_title(output_title.as_str().into());
    app.set_output_text(output_text.as_str().into());

    app.set_agent_open(state.agent_open);
    app.set_agent_lines(ModelRc::new(VecModel::from(projection::agent_lines(ed))));
    app.set_agent_running(projection::agent_running(ed));
    app.set_changes(ModelRc::new(VecModel::from(projection::changes(ed))));
    let (debug_active, debug_frames, debug_console) = projection::debug_state(ed);
    app.set_debug_active(debug_active);
    app.set_debug_frames(ModelRc::new(VecModel::from(
        debug_frames.into_iter().map(Into::into).collect::<Vec<slint::SharedString>>(),
    )));
    app.set_debug_console(debug_console.as_str().into());
    app.set_recents(ModelRc::new(VecModel::from(
        projection::recent_projects(ed)
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));
    app.set_show_welcome(ed.project_manager.project.root.is_none());
    app.set_status_mode(projection::status_mode(ed).as_str().into());

    app.set_menu_visible(state.menu_visible);
    app.set_menu_x(state.menu_x);
    let items = state
        .menu_title
        .map(projection::menu_items)
        .map(|items| ModelRc::new(VecModel::from(items)))
        .unwrap_or_else(|| ModelRc::new(VecModel::from(Vec::new())));
    app.set_menu_items(items);

    // ── Hover card sync ────────────────────────────────────────────────────
    {
        let busy = ed.mouse_state.pressed || ed.mouse_state.drag_active || ed.completion.visible;
        if state.hovering && !busy {
            let focused_pane = ed.view_tree.focused_window()
                .and_then(|wid| ed.view_tree.window(wid).cloned());
            let focused_buf = focused_pane.as_ref().and_then(|p| p.buffer_id);
            let cell_x = (state.last_px / pane::CHAR_W as f32).max(0.0) as u16;
            let cell_y = (state.last_py / pane::LINE_H as f32).max(0.0) as u16;
            let target = if let (Some(pane), Some(buf_id)) = (&focused_pane, focused_buf) {
                magma::kernel::input::mouse::resolve_click_to_offset(ed, buf_id, pane, cell_x, cell_y)
                    .and_then(|offset| {
                        let arc = ed.buffers.get(buf_id)?;
                        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                        let (ws, we) = magma::kernel::input::mouse::word_range_at(&buf, offset)?;
                        let diag_line = buf.position_of(offset).line;
                        let diag = buf.get_diagnostics().into_iter()
                            .find(|d| d.line == diag_line)
                            .map(|d| d.display());
                        Some((buf.slice(ws, we), diag, offset, buf_id))
                    })
            } else { None };

            let same_word = matches!(
                (&target, ed.hover.as_ref()),
                (Some((w, _, _, _)), Some(card)) if &card.word == w
            );
            if !same_word
                && let Some((_, _, offset, buf_id)) = &target {
                    // request LSP hover
                    if let Some(lang) = ed.lsp.language_for_buffer(*buf_id)
                        && let Some(server) = ed.lsp.get(&lang)
                        && let Some(arc) = ed.buffers.get(*buf_id) {
                            let (uri, position, revision) = {
                                let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                                match buf.path.as_deref() {
                                    Some(path) => (format!("file://{path}"), buf.lsp_position((*offset).min(buf.len())), buf.revision()),
                                    None => (String::new(), (0, 0), buf.revision()),
                                }
                            };
                            if !uri.is_empty() {
                                let params = serde_json::json!({
                                    "textDocument": { "uri": uri },
                                    "position": { "line": position.0, "character": position.1 },
                                });
                                server.request("textDocument/hover", params, *buf_id, revision);
                            }
                        }
                }

            let new_card = match (&target, ed.hover.as_ref()) {
                (None, _) => None,
                (Some(_), Some(card)) if Some(&card.word) == target.as_ref().map(|t| &t.0) => None,
                (Some((w, diag, _, _)), _) => Some(magma::kernel::state::HoverCard {
                    word: w.clone(),
                    px: state.last_px,
                    py: state.last_py,
                    since: std::time::Instant::now(),
                    lines: {
                        let mut lines = Vec::new();
                        let defs: Vec<(String, &magma::kernel::index::extract::IndexedSymbol)> =
                            ed.index.symbols_with_prefix(w)
                                .into_iter()
                                .filter(|(_, sym)| sym.name == *w)
                                .take(3)
                                .collect();
                        if defs.is_empty() {
                            lines.push(format!("{w} — no indexed definition"));
                        }
                        for (path, sym) in defs {
                            let parts: Vec<&str> = path.rsplit('/').take(2).collect();
                            let short = parts.iter().rev().copied().collect::<Vec<_>>().join("/");
                            lines.push(format!("{} {w}  ·  {short}:{}", sym.kind, sym.line + 1));
                        }
                        if let Some(d) = diag { lines.push(d.clone()); }
                        lines
                    },
                }),
            };
            if target.is_none() {
                ed.hover = None;
            } else if let Some(card) = new_card {
                ed.hover = Some(card);
            }
        } else if !state.hovering {
            ed.hover = None;
        }
    }

    // sync hover-visible
    {
        let show = ed.hover.as_ref().map(|c| c.since.elapsed() >= std::time::Duration::from_millis(450) && !c.lines.is_empty()).unwrap_or(false);
        app.set_hover_visible(show);
        if let Some(card) = &ed.hover {
            app.set_hover_x(card.px as f32 + 14.0);
            app.set_hover_y(card.py as f32 + 20.0);
            let lines: Vec<slint::SharedString> = card.lines.iter().map(|l| l.as_str().into()).collect();
            app.set_hover_lines(slint::ModelRc::new(slint::VecModel::from(lines)));
        }
    }

    // ── Completion popup sync ───────────────────────────────────────────────
    {
        let show = ed.completion.visible && !ed.completion.items.is_empty();
        app.set_completion_visible(show);
        if show {
            // position at cursor (rough: use focused pane cursor)
            let focused_pane = ed.view_tree.focused_window().and_then(|wid| ed.view_tree.window(wid).cloned());
            let (cx, cy) = if let Some(pane) = &focused_pane {
                let buf_id = pane.buffer_id.unwrap_or(0);
                let visible_rows = pane.height as usize;
                let st = magma::kernel::render::frame::pane_scroll_state(ed, pane, buf_id, visible_rows.max(1));
                let header_rows = if ed.buffers.get(buf_id).map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).header_line.is_some()).unwrap_or(false) { 1 } else { 0 };
                let total_lines = ed.buffers.get(buf_id).map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count()).unwrap_or(1);
                let gutter = magma::kernel::render::frame::gutter_cols(ed, buf_id, total_lines);
                let x = (gutter as f32 + st.cursor_col as f32) * pane::CHAR_W as f32;
                let y = (header_rows as f32 + st.cursor_vis_row as f32) * pane::LINE_H as f32;
                (x, y)
            } else { (0.0, 0.0) };
            app.set_completion_x(cx);
            app.set_completion_y(cy + pane::LINE_H as f32);
            let items: Vec<slint::SharedString> = ed.completion.items.iter().take(10).map(|i| i.label.as_str().into()).collect();
            app.set_completion_items(slint::ModelRc::new(slint::VecModel::from(items)));
            app.set_completion_selected(ed.completion.idx as i32);
        }
    }

    // ── Quickfix menu sync ──────────────────────────────────────────────────
    {
        let show = ed.quickfix_menu.as_ref().map(|m| !m.items.is_empty()).unwrap_or(false);
        app.set_quickfix_visible(show);
        if show {
            let menu = ed.quickfix_menu.as_ref().unwrap();
            let items: Vec<slint::SharedString> = menu.items.iter().take(12).map(|a| a.title.as_str().into()).collect();
            app.set_quickfix_items(slint::ModelRc::new(slint::VecModel::from(items)));
            app.set_quickfix_selected(menu.idx as i32);
            // position at cursor
            let focused_pane = ed.view_tree.focused_window().and_then(|wid| ed.view_tree.window(wid).cloned());
            let (qx, qy) = if let Some(pane) = &focused_pane {
                let buf_id = pane.buffer_id.unwrap_or(0);
                let visible_rows = pane.height as usize;
                let st = magma::kernel::render::frame::pane_scroll_state(ed, pane, buf_id, visible_rows.max(1));
                let header_rows = if ed.buffers.get(buf_id).map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).header_line.is_some()).unwrap_or(false) { 1 } else { 0 };
                let total_lines = ed.buffers.get(buf_id).map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count()).unwrap_or(1);
                let gutter = magma::kernel::render::frame::gutter_cols(ed, buf_id, total_lines);
                let x = (gutter as f32 + st.cursor_col as f32) * pane::CHAR_W as f32;
                let y = (header_rows as f32 + st.cursor_vis_row as f32) * pane::LINE_H as f32;
                (x, y + pane::LINE_H as f32)
            } else { (0.0, 0.0) };
            app.set_quickfix_x(qx);
            app.set_quickfix_y(qy);
        }
    }

    // ── Minimap sync ───────────────────────────────────────────────────────
    {
        let enabled = ed.options.get("minimap-enabled").map(|v| v.as_str()) == Some("true");
        app.set_minimap_visible(enabled);
        if enabled {
            let focused_pane = ed.view_tree.focused_window().and_then(|wid| ed.view_tree.window(wid).cloned());
            if let Some(pane) = &focused_pane
                && let Some(buf_id) = pane.buffer_id {
                    let visible_rows = pane.height as usize;
                    let scroll_top = magma::kernel::render::frame::pane_scroll_top(ed, pane, buf_id, visible_rows.max(1));
                    let img = pane::blit_minimap(ed, buf_id, pane::MINIMAP_W, pane.height as u32 * pane::LINE_H, scroll_top, visible_rows);
                    app.set_minimap(img);
                }
        }
    }

    // ── Dialog sync ────────────────────────────────────────────────────────
    app.set_dialog_visible(state.dialog_visible);
    app.set_dialog_title(state.dialog_title.as_str().into());
    // settings visible is in-out: read user changes back
    state.settings_visible = app.get_settings_visible();

    // ── Cursor position for IME overlay ────────────────────────────────────
    {
        let focused_pane = ed.view_tree.focused_window().and_then(|wid| ed.view_tree.window(wid).cloned());
        let (cx, cy) = if let Some(pane) = &focused_pane {
            let buf_id = pane.buffer_id.unwrap_or(0);
            let visible_rows = pane.height as usize;
            let st = magma::kernel::render::frame::pane_scroll_state(ed, pane, buf_id, visible_rows.max(1));
            let header_rows = if ed.buffers.get(buf_id).map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).header_line.is_some()).unwrap_or(false) { 1 } else { 0 };
            let total_lines = ed.buffers.get(buf_id).map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count()).unwrap_or(1);
            let gutter = magma::kernel::render::frame::gutter_cols(ed, buf_id, total_lines);
            let x = (gutter as f32 + st.cursor_col as f32) * pane::CHAR_W as f32;
            let y = (header_rows as f32 + st.cursor_vis_row as f32) * pane::LINE_H as f32;
            (x, y)
        } else { (0.0, 0.0) };
        app.set_cursor_x(cx);
        app.set_cursor_y(cy);
    }

    state.rendered_revision = revision_of(ed);
}

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
    theme::apply_slint(app, &palette);
    theme::apply_editor(&mut state.editor, &palette);
    state.theme_name = name;
}

fn sync_tree(app: &AppWindow, state: &mut AppState) {
    let project = &state.editor.project_manager.project;
    let root = project.root.clone();
    if state.tree_epoch != project.files_epoch || state.tree_root != root {
        state.tree_epoch = project.files_epoch;
        state.tree_root = root;
        state.tree_nodes = build(&project.files);
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
        projection::tree_rows(&state.tree_nodes, &state.expanded)
    } else {
        Vec::new()
    };
    app.set_tree(ModelRc::new(VecModel::from(rows)));
    app.set_selected_path(projection::current_path(&state.editor).into());
}
