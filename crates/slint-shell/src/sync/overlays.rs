//! Overlay sync for the refresh pass: hover card, completion, quickfix,
//! minimap, breadcrumb option, dialog chrome, and the IME cursor anchor.

use crate::{AppState, AppWindow, pane};

/// Logical (x, y) of the focused pane's caret in editor-view coordinates.
/// The header row and gutter offset the origin, and horizontal scroll pans
/// the column — the same math the kernel render pass uses to stamp the caret.
/// `row_w`/`row_h` are the device-true logical cell units from
/// [`pane::logical_cell`]: the blitted image lays rows out at
/// `round(cell × ppx) / ppx`, so anchoring on the raw integer grid drifts a
/// fraction of a pixel per row until the caret visibly leaves its line.
fn cursor_anchor(ed: &kernel::state::Editor, row_w: f32, row_h: f32) -> (f32, f32) {
    let Some(pane) = ed
        .view_tree
        .focused_window()
        .and_then(|wid| ed.view_tree.window(wid))
    else {
        return (0.0, 0.0);
    };
    let buf_id = pane.buffer_id.unwrap_or(0);
    let visible_rows = (pane.height as usize).max(1);
    let st = kernel::render::frame::pane_scroll_state(ed, pane, buf_id, visible_rows);
    let header_rows = usize::from(ed.buffers.get(buf_id).is_some_and(|a| {
        a.lock()
            .unwrap_or_else(|e| e.into_inner())
            .header_line
            .is_some()
    }));
    let total_lines = ed
        .buffers
        .get(buf_id)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
        .unwrap_or(1);
    let gutter = kernel::render::frame::gutter_cols(ed, buf_id, total_lines);
    let scroll_left = kernel::render::frame::pane_scroll_left(pane, buf_id);
    let x = (gutter + st.cursor_col.saturating_sub(scroll_left)) as f32 * row_w;
    let y = (header_rows as f32 + st.cursor_vis_row as f32) * row_h;
    (x, y)
}

fn hash_values(values: &[u64]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for v in values {
        h ^= *v;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

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
                            // The server knows the document by the encoded
                            // didOpen URI — a raw path never matches.
                            Some(path) => (
                                kernel::semantic::lsp_proto::path_to_uri(path),
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
            state.m_hover_lines.set(lines, |m| app.set_hover_lines(m));
        }
    }

    // ── Completion popup sync ───────────────────────────────────────────────
    {
        let show = ed.completion.visible && !ed.completion.items.is_empty();
        app.set_completion_visible(show);
        if show {
            let (row_w, row_h) = pane::logical_cell(state.cell_w, state.cell_h, state.atlas_ppx);
            let (cx, cy) = cursor_anchor(ed, row_w, row_h);
            app.set_completion_x(cx);
            app.set_completion_y(cy + row_h);
            // The overlay shows a scrolling window of the item list so
            // keyboard navigation stays visible past the first page.  Each
            // row carries its kind and signature; the info pane shows the
            // selected entry's signature plus documentation.
            let (start, window) =
                kernel::completion::popup_window(&ed.completion.items, ed.completion.idx);
            let entries: Vec<crate::CompletionEntry> = window
                .iter()
                .map(|i| crate::CompletionEntry {
                    label: i.label.as_str().into(),
                    kind: i.kind.as_str().into(),
                    detail: i.detail.as_str().into(),
                })
                .collect();
            state
                .m_completion_entries
                .set(entries, |m| app.set_completion_entries(m));
            app.set_completion_selected((ed.completion.idx - start) as i32);
            let selected = ed.completion.items.get(ed.completion.idx);
            let mut info = selected.map(|i| i.detail.clone()).unwrap_or_default();
            let docs = selected.map(|i| i.docs.clone()).unwrap_or_default();
            if !docs.is_empty() {
                if !info.is_empty() {
                    info.push('\n');
                }
                info.push_str(&docs);
            }
            app.set_completion_info(info.as_str().into());
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
            state
                .m_quickfix_items
                .set(items, |m| app.set_quickfix_items(m));
            app.set_quickfix_selected(menu.idx as i32);
            let (row_w, row_h) = pane::logical_cell(state.cell_w, state.cell_h, state.atlas_ppx);
            let (qx, qy) = cursor_anchor(ed, row_w, row_h);
            app.set_quickfix_x(qx);
            app.set_quickfix_y(qy + row_h);
        }
    }

    // ── Minimap + breadcrumb sync ──────────────────────────────────────────
    let breadcrumb_enabled =
        ed.options.get("breadcrumb-enabled").map(|v| v.as_str()) != Some("false");
    app.set_breadcrumb_enabled(breadcrumb_enabled);
    {
        let enabled = ed.options.get("minimap-enabled").map(|v| v.as_str()) == Some("true");
        app.set_minimap_visible(enabled);
        if enabled
            && let Some(pane) = ed
                .view_tree
                .focused_window()
                .and_then(|wid| ed.view_tree.window(wid))
            && let Some(buf_id) = pane.buffer_id
        {
            let visible_rows = pane.height as usize;
            let scroll_top =
                kernel::render::frame::pane_scroll_top(ed, pane, buf_id, visible_rows.max(1));
            let revision = ed
                .buffers
                .get(buf_id)
                .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).revision())
                .unwrap_or(0);
            // The Image element is logical; blit at device resolution so the
            // map is not resampled on high-DPI displays.
            let ppx = state.atlas_ppx.max(1.0);
            let width = (pane::MINIMAP_W as f32 * ppx).round() as u32;
            let height = ((pane.height as u32 * state.cell_h) as f32 * ppx).round() as u32;
            let sig = hash_values(&[
                buf_id as u64,
                revision,
                scroll_top as u64,
                visible_rows as u64,
                width as u64,
                height as u64,
            ]);
            if sig != state.minimap_sig {
                let img = pane::blit_minimap(ed, buf_id, width, height, scroll_top, visible_rows);
                app.set_minimap(img);
                state.minimap_sig = sig;
            }
        }
    }
    // ── Cursor position for IME overlay + thin caret overlay ──────────────
    {
        let focused = ed.view_tree.focused_window().is_some();
        let (row_w, row_h) = pane::logical_cell(state.cell_w, state.cell_h, state.atlas_ppx);
        let (cx, cy) = cursor_anchor(ed, row_w, row_h);
        app.set_cursor_x(cx);
        // The cell's top leading is empty: glyphs hang from a baseline at the
        // row's ascent, so the beam starts at the cap line or it reads as
        // sitting above the text.  Both the inset and the beam height are
        // measured from the atlas pixels (sync's atlas-rebuild pass), so the
        // beam covers exactly the capital's ink span — no more, no less.
        let caret_h = row_h * 0.75;
        app.set_cursor_y(cy + (row_h - caret_h) / 2.0);
        app.set_cursor_height(caret_h.max(1.0));
        app.set_caret_visible(focused && state.caret_blink_on);
        let (cr, cg, cb) = ed.theme_color("cursor");
        app.set_caret_color(slint::Color::from_rgb_u8(cr, cg, cb));
    }
}
