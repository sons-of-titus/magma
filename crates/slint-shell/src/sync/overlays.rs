//! Overlay sync for the refresh pass: hover card, completion, quickfix,
//! minimap, breadcrumb option, dialog chrome, and the IME cursor anchor.

use crate::{AppState, AppWindow, pane};

pub(super) fn sync(app: &AppWindow, state: &mut AppState) {
    let ed = &mut state.editor;
    // ── Hover card sync ────────────────────────────────────────────────────
    {
        let busy = ed.mouse_state.pressed || ed.mouse_state.drag_active || ed.completion.visible;
        if state.hovering && !busy {
            let focused_pane = ed
                .view_tree
                .focused_window()
                .and_then(|wid| ed.view_tree.window(wid).cloned());
            let focused_buf = focused_pane.as_ref().and_then(|p| p.buffer_id);
            let cell_x = (state.last_px / state.cell_w as f32).max(0.0) as u16;
            let cell_y = (state.last_py / state.cell_h as f32).max(0.0) as u16;
            let target = if let (Some(pane), Some(buf_id)) = (&focused_pane, focused_buf) {
                kernel::input::mouse::resolve_click_to_offset(ed, buf_id, pane, cell_x, cell_y)
                    .and_then(|offset| {
                        let arc = ed.buffers.get(buf_id)?;
                        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                        let (ws, we) = kernel::input::mouse::word_range_at(&buf, offset)?;
                        let diag_line = buf.position_of(offset).line;
                        let diag = buf
                            .get_diagnostics()
                            .into_iter()
                            .find(|d| d.line == diag_line)
                            .map(|d| d.display());
                        Some((buf.slice(ws, we), diag, offset, buf_id))
                    })
            } else {
                None
            };

            let same_word = matches!(
                (&target, ed.hover.as_ref()),
                (Some((w, _, _, _)), Some(card)) if &card.word == w
            );
            if !same_word && let Some((_, _, offset, buf_id)) = &target {
                // request LSP hover
                if let Some(lang) = ed.lsp.language_for_buffer(*buf_id)
                    && let Some(server) = ed.lsp.get(&lang)
                    && let Some(arc) = ed.buffers.get(*buf_id)
                {
                    let (uri, position, revision) = {
                        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                        match buf.path.as_deref() {
                            Some(path) => (
                                format!("file://{path}"),
                                buf.lsp_position((*offset).min(buf.len())),
                                buf.revision(),
                            ),
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
                (Some((w, diag, _, _)), _) => Some(kernel::state::HoverCard {
                    word: w.clone(),
                    px: state.last_px,
                    py: state.last_py,
                    since: std::time::Instant::now(),
                    lines: {
                        let mut lines = Vec::new();
                        let defs: Vec<(String, &kernel::index::extract::IndexedSymbol)> = ed
                            .index
                            .symbols_with_prefix(w)
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
                        if let Some(d) = diag {
                            lines.push(d.clone());
                        }
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
        let show = ed
            .hover
            .as_ref()
            .map(|c| {
                c.since.elapsed() >= std::time::Duration::from_millis(450) && !c.lines.is_empty()
            })
            .unwrap_or(false);
        app.set_hover_visible(show);
        if let Some(card) = &ed.hover {
            app.set_hover_x(card.px as f32 + 14.0);
            app.set_hover_y(card.py as f32 + 20.0);
            let lines: Vec<slint::SharedString> =
                card.lines.iter().map(|l| l.as_str().into()).collect();
            app.set_hover_lines(slint::ModelRc::new(slint::VecModel::from(lines)));
        }
    }

    // ── Completion popup sync ───────────────────────────────────────────────
    {
        let show = ed.completion.visible && !ed.completion.items.is_empty();
        app.set_completion_visible(show);
        if show {
            // position at cursor (rough: use focused pane cursor)
            let focused_pane = ed
                .view_tree
                .focused_window()
                .and_then(|wid| ed.view_tree.window(wid).cloned());
            let (cx, cy) = if let Some(pane) = &focused_pane {
                let buf_id = pane.buffer_id.unwrap_or(0);
                let visible_rows = pane.height as usize;
                let st =
                    kernel::render::frame::pane_scroll_state(ed, pane, buf_id, visible_rows.max(1));
                let header_rows = if ed
                    .buffers
                    .get(buf_id)
                    .map(|a| {
                        a.lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .header_line
                            .is_some()
                    })
                    .unwrap_or(false)
                {
                    1
                } else {
                    0
                };
                let total_lines = ed
                    .buffers
                    .get(buf_id)
                    .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
                    .unwrap_or(1);
                let gutter = kernel::render::frame::gutter_cols(ed, buf_id, total_lines);
                let x = (gutter as f32 + st.cursor_col as f32) * state.cell_w as f32;
                let y = (header_rows as f32 + st.cursor_vis_row as f32) * state.cell_h as f32;
                (x, y)
            } else {
                (0.0, 0.0)
            };
            app.set_completion_x(cx);
            app.set_completion_y(cy + state.cell_h as f32);
            let items: Vec<slint::SharedString> = ed
                .completion
                .items
                .iter()
                .take(10)
                .map(|i| i.label.as_str().into())
                .collect();
            app.set_completion_items(slint::ModelRc::new(slint::VecModel::from(items)));
            app.set_completion_selected(ed.completion.idx as i32);
        }
    }

    // ── Quickfix menu sync ──────────────────────────────────────────────────
    {
        let show = ed
            .quickfix_menu
            .as_ref()
            .map(|m| !m.items.is_empty())
            .unwrap_or(false);
        app.set_quickfix_visible(show);
        if show {
            let menu = ed.quickfix_menu.as_ref().unwrap();
            let items: Vec<slint::SharedString> = menu
                .items
                .iter()
                .take(12)
                .map(|a| a.title.as_str().into())
                .collect();
            app.set_quickfix_items(slint::ModelRc::new(slint::VecModel::from(items)));
            app.set_quickfix_selected(menu.idx as i32);
            // position at cursor
            let focused_pane = ed
                .view_tree
                .focused_window()
                .and_then(|wid| ed.view_tree.window(wid).cloned());
            let (qx, qy) = if let Some(pane) = &focused_pane {
                let buf_id = pane.buffer_id.unwrap_or(0);
                let visible_rows = pane.height as usize;
                let st =
                    kernel::render::frame::pane_scroll_state(ed, pane, buf_id, visible_rows.max(1));
                let header_rows = if ed
                    .buffers
                    .get(buf_id)
                    .map(|a| {
                        a.lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .header_line
                            .is_some()
                    })
                    .unwrap_or(false)
                {
                    1
                } else {
                    0
                };
                let total_lines = ed
                    .buffers
                    .get(buf_id)
                    .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
                    .unwrap_or(1);
                let gutter = kernel::render::frame::gutter_cols(ed, buf_id, total_lines);
                let x = (gutter as f32 + st.cursor_col as f32) * state.cell_w as f32;
                let y = (header_rows as f32 + st.cursor_vis_row as f32) * state.cell_h as f32;
                (x, y + state.cell_h as f32)
            } else {
                (0.0, 0.0)
            };
            app.set_quickfix_x(qx);
            app.set_quickfix_y(qy);
        }
    }

    // ── Minimap + breadcrumb sync ──────────────────────────────────────────
    let breadcrumb_enabled =
        ed.options.get("breadcrumb-enabled").map(|v| v.as_str()) != Some("false");
    app.set_breadcrumb_enabled(breadcrumb_enabled);
    {
        let enabled = ed.options.get("minimap-enabled").map(|v| v.as_str()) == Some("true");
        app.set_minimap_visible(enabled);
        if enabled {
            let focused_pane = ed
                .view_tree
                .focused_window()
                .and_then(|wid| ed.view_tree.window(wid).cloned());
            if let Some(pane) = &focused_pane
                && let Some(buf_id) = pane.buffer_id
            {
                let visible_rows = pane.height as usize;
                let scroll_top =
                    kernel::render::frame::pane_scroll_top(ed, pane, buf_id, visible_rows.max(1));
                let img = pane::blit_minimap(
                    ed,
                    buf_id,
                    pane::MINIMAP_W,
                    pane.height as u32 * state.cell_h,
                    scroll_top,
                    visible_rows,
                );
                app.set_minimap(img);
            }
        }
    }
    // ── Cursor position for IME overlay ────────────────────────────────────
    {
        let focused_pane = ed
            .view_tree
            .focused_window()
            .and_then(|wid| ed.view_tree.window(wid).cloned());
        let (cx, cy) = if let Some(pane) = &focused_pane {
            let buf_id = pane.buffer_id.unwrap_or(0);
            let visible_rows = pane.height as usize;
            let st =
                kernel::render::frame::pane_scroll_state(ed, pane, buf_id, visible_rows.max(1));
            let header_rows = if ed
                .buffers
                .get(buf_id)
                .map(|a| {
                    a.lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .header_line
                        .is_some()
                })
                .unwrap_or(false)
            {
                1
            } else {
                0
            };
            let total_lines = ed
                .buffers
                .get(buf_id)
                .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
                .unwrap_or(1);
            let gutter = kernel::render::frame::gutter_cols(ed, buf_id, total_lines);
            let x = (gutter as f32 + st.cursor_col as f32) * state.cell_w as f32;
            let y = (header_rows as f32 + st.cursor_vis_row as f32) * state.cell_h as f32;
            (x, y)
        } else {
            (0.0, 0.0)
        };
        app.set_cursor_x(cx);
        app.set_cursor_y(cy);
    }
}
