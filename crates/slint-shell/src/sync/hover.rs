//! Hover card sync: pointer target resolution, dwell timing, content build,
//! and viewport-aware placement — the card flips over the opposite edge
//! instead of clipping against the window.

use super::geometry::{count_word_occurrences, hover_card_geometry, hover_card_position, view_size};
use crate::{AppState, AppWindow};

/// The card appears after this much pointer rest on one word.
const DWELL_MS: u64 = 300;
/// Moving over whitespace keeps an existing card alive this long.
const GRACE_MS: u64 = 300;
/// Re-entering a recently seen word re-shows its card instantly.
const RESHOW_MS: u64 = 1500;

/// The word under the pointer plus everything card building and plugin
/// augmentation need.
struct HoverTarget {
    word: String,
    diag: Option<String>,
    offset: usize,
    buf_id: usize,
    /// Full text of the hovered line (newline included).
    line_text: String,
    line: usize,
    path: String,
}

pub(super) fn sync(app: &AppWindow, state: &mut AppState) {
    let ed = &mut state.editor;
    let (view_w, view_h) = view_size(app);
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
                .and_then(|offset| hover_target(ed, buf_id, offset))
        } else {
            None
        };

        // Pinning: keep an existing card alive while the pointer is over it
        // or briefly over whitespace on its way to it; dismiss once the
        // pointer rests on whitespace away from the card.
        let mut skip_update = false;
        if let Some(card) = ed.hover.clone() {
            let (cw, ch) = hover_card_geometry(card.lines.len());
            let (cx, cy) = hover_card_position(card.px, card.py, cw, ch, view_w, view_h);
            let over_card = state.last_px >= cx
                && state.last_px <= cx + cw
                && state.last_py >= cy
                && state.last_py <= cy + ch;
            if over_card {
                state.hover_none_since = None;
                skip_update = true;
            } else if target.is_none() {
                let now = std::time::Instant::now();
                let since = state.hover_none_since.get_or_insert(now);
                if since.elapsed() < std::time::Duration::from_millis(GRACE_MS) {
                    skip_update = true;
                } else {
                    ed.hover = None;
                    state.hover_none_since = None;
                    skip_update = true;
                }
            } else {
                state.hover_none_since = None;
            }
        } else {
            state.hover_none_since = None;
        }

        let same_word = matches!(
            (&target, ed.hover.as_ref()),
            (Some(t), Some(card)) if card.word == t.word
        );
        if !skip_update {
            if !same_word && let Some(t) = &target {
                request_lsp_hover(ed, t);
            }

            let new_card = match (&target, ed.hover.as_ref()) {
                (None, _) => None,
                (Some(_), Some(_)) if same_word => None,
                (Some(t), _) => {
                    // A word whose card was on screen moments ago shows
                    // instantly: sweeping back and forth between two names
                    // must not re-pay the dwell on every crossing.
                    let now = std::time::Instant::now();
                    let instant = matches!(
                        &state.hover_seen,
                        Some((w, b, seen)) if *w == t.word
                            && *b == t.buf_id
                            && seen.elapsed() < std::time::Duration::from_millis(RESHOW_MS)
                    );
                    let since = if instant {
                        now.checked_sub(std::time::Duration::from_millis(DWELL_MS))
                            .unwrap_or(now)
                    } else {
                        now
                    };
                    Some(build_card(ed, t, state.last_px, state.last_py, since))
                }
            };
            if target.is_none() {
                ed.hover = None;
            } else if let Some(card) = new_card {
                let t = target.as_ref().unwrap();
                // Language plugins may augment the card (markdown link
                // targets, heading context); the payload reaches `event/on`
                // subscribers on the next refresh's event dispatch.
                ed.events.emit_typed(
                    kernel::event::keys::events::HOVER_CARD,
                    kernel::event::payload::HoverCardPayload {
                        word: t.word.clone(),
                        line_text: t.line_text.clone(),
                        line: t.line.to_string(),
                        path: t.path.clone(),
                    },
                );
                ed.hover = Some(card);
            }
        }
    } else if !state.hovering {
        ed.hover = None;
    }

    // Visibility + placement.  A card exists before it is visible (dwell
    // gate), and x/y/height keep tracking it so a flip recalculates when
    // the content grows with the LSP response.
    {
        let show = ed
            .hover
            .as_ref()
            .map(|c| {
                c.since.elapsed() >= std::time::Duration::from_millis(DWELL_MS)
                    && !c.lines.is_empty()
            })
            .unwrap_or(false);
        app.set_hover_visible(show);
        if let Some(card) = &ed.hover {
            let (cw, ch) = hover_card_geometry(card.lines.len());
            let (x, y) = hover_card_position(card.px, card.py, cw, ch, view_w, view_h);
            app.set_hover_x(x);
            app.set_hover_y(y);
            app.set_hover_height(ch);
            let lines: Vec<crate::HoverLine> = card
                .lines
                .iter()
                .map(|l| crate::HoverLine {
                    text: l.text.as_str().into(),
                    kind: l.kind.as_str().into(),
                })
                .collect();
            state.m_hover_lines.set(lines, |m| app.set_hover_lines(m));
            state.hover_seen = Some((card.word.clone(), card.buf_key, std::time::Instant::now()));
        }
    }
}

fn hover_target(ed: &mut kernel::state::Editor, buf_id: usize, offset: usize) -> Option<HoverTarget> {
    let arc = ed.buffers.get(buf_id)?;
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    let (ws, we) = kernel::input::mouse::word_range_at(&buf, offset)?;
    let line = buf.position_of(offset).line;
    let start = buf.line_start_offset(line).unwrap_or(0);
    let line_text = buf.slice(start, buf.line_start_offset(line + 1).unwrap_or(buf.len()));
    // Only hover when the pointer sits directly on a word character, so
    // trailing whitespace never resurrects the previous word's card.  The
    // line text is indexed by byte offset: indexing chars by byte offset
    // misread any line with a multi-byte character before the pointer and
    // silently killed hover on it.
    let cur = offset
        .checked_sub(start)
        .and_then(|i| line_text.get(i..))
        .and_then(|s| s.chars().next());
    if !cur.is_some_and(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    let diag = buf
        .get_diagnostics()
        .into_iter()
        .find(|d| d.line == line)
        .map(|d| d.display());
    let path = buf.path.clone().unwrap_or_default();
    Some(HoverTarget {
        word: buf.slice(ws, we),
        diag,
        offset,
        buf_id,
        line_text,
        line,
        path,
    })
}

fn request_lsp_hover(ed: &mut kernel::state::Editor, t: &HoverTarget) {
    if let Some(lang) = ed.lsp.language_for_buffer(t.buf_id)
        && let Some(server) = ed.lsp.get(&lang)
        && let Some(arc) = ed.buffers.get(t.buf_id)
    {
        let (uri, position, revision) = {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            match buf.path.as_deref() {
                // The server knows the document by the encoded didOpen
                // URI — a raw path never matches.
                Some(path) => (
                    kernel::semantic::lsp_proto::path_to_uri(path),
                    buf.lsp_position(t.offset.min(buf.len())),
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
            server.request("textDocument/hover", params, t.buf_id, revision);
        }
    }
}

fn build_card(
    ed: &kernel::state::Editor,
    t: &HoverTarget,
    px: f32,
    py: f32,
    since: std::time::Instant,
) -> kernel::state::HoverCard {
    let mut lines = Vec::new();
    let defs: Vec<(String, &kernel::index::extract::IndexedSymbol)> = ed
        .index
        .symbols_with_prefix(&t.word)
        .into_iter()
        .filter(|(_, sym)| sym.name == t.word)
        .take(3)
        .collect();
    let indexed = !defs.is_empty();
    for (path, sym) in defs {
        let parts: Vec<&str> = path.rsplit('/').take(2).collect();
        let short = parts.iter().rev().copied().collect::<Vec<_>>().join("/");
        lines.push(kernel::state::HoverLine {
            kind: "muted".into(),
            text: format!("{} {}  ·  {short}:{}", sym.kind, t.word, sym.line + 1),
        });
    }
    if !indexed {
        // No index hit: the file itself is the context.  An occurrence
        // count beats "no indexed definition" for prose files.
        let (count, file) = ed
            .buffers
            .get(t.buf_id)
            .map(|arc| {
                let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                let full = buf.slice(0, buf.len());
                let name = buf
                    .path
                    .as_deref()
                    .and_then(|p| p.rsplit('/').next())
                    .unwrap_or("unnamed")
                    .to_string();
                (count_word_occurrences(&full, &t.word), name)
            })
            .unwrap_or((0, "unnamed".to_string()));
        lines.push(kernel::state::HoverLine {
            kind: "muted".into(),
            text: format!("{} · {count} occurrences in {file}", t.word),
        });
    }
    if let Some(d) = &t.diag {
        lines.push(kernel::state::HoverLine {
            kind: "muted".into(),
            text: d.clone(),
        });
    }
    kernel::state::HoverCard {
        word: t.word.clone(),
        buf_key: t.buf_id,
        offset: t.offset,
        px,
        py,
        since,
        lines,
    }
}
