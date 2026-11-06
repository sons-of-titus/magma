//! Terminal dock sync: session tabs, the styled scrollback image, and the
//! signature gate that skips re-blitting unchanged output.
//!
//! Split from `sync/mod.rs` to keep the per-tick projection under the
//! file-size gate.

use std::hash::{Hash, Hasher};

use crate::{AppState, AppWindow, pane, theme};

pub(super) fn sync(app: &AppWindow, state: &mut AppState, logical_w: f32, ppx: f32, dock_h: f32) {
    let ed = &mut state.editor;
    // terminal sessions
    {
        let sessions: Vec<slint::SharedString> = ed
            .io
            .terminals
            .sessions
            .iter()
            .map(|s| s.title.as_str().into())
            .collect();
        state
            .m_sessions
            .set(sessions, |m| app.set_terminal_sessions(m));
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
        let term_view_h = (dock_h - 35.0).max(1.0);
        let (term_cols, term_rows) =
            pane::terminal_grid_size(logical_w, session_count, state.cell_w, state.cell_h, term_view_h);
        app.set_terminal_body_height(term_rows as f32 * state.cell_h as f32);
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
                state.terminal_sig = 0;
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
                // A full device-resolution blit plus texture upload only pays
                // for itself when the rendered rows actually changed.
                let content_key = format!("{:?}{cursor:?}{}", lines, state.theme_name);
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                content_key.hash(&mut hasher);
                let sig = hasher.finish();
                if sig != state.terminal_sig {
                    let chain = ed.font_config.font_chain();
                    state.atlas.ensure_glyphs(
                        lines.iter().flat_map(|l| l.text.chars()),
                        &chain,
                    );
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
                    state.terminal_sig = sig;
                }
            }
        } else if state.bottom == 0 {
            app.set_terminal_image(slint::Image::default());
        }
    }
}
