//! Signature help — one line above the caret showing the active call
//! signature with the active parameter highlighted.

use crate::render::surface::{Style, Surface};
use crate::state::Editor;

/// Draw the signature line anchored above the caret (`caret` is the
/// caret's absolute surface cell).  On the first visible row it draws
/// below the caret instead of off the top.
pub fn render(editor: &Editor, surface: &mut Surface, caret: (u16, u16)) {
    if !editor.signature.visible {
        return;
    }
    let Some(sig) = editor.signature.active() else {
        return;
    };
    let label: String = sig.label.chars().take(120).collect();
    let width = label.chars().count() as u16;
    if width == 0 || width + 2 > surface.width {
        return;
    }
    let row = if caret.1 > 0 {
        caret.1 - 1
    } else {
        caret.1 + 1
    };
    let x = caret.0.min(surface.width.saturating_sub(width + 1));
    let base = Style {
        fg: (210, 210, 210),
        bg: (35, 35, 40),
        ..Default::default()
    };
    let highlight = Style {
        fg: (255, 210, 120),
        bg: (70, 55, 15),
        ..Default::default()
    };
    for cx in 0..width + 2 {
        if x + cx < surface.width {
            surface.set_cell(x + cx, row, ' ', Some(base));
        }
    }
    // Parameter ranges are byte offsets into the full label; clamp to the
    // truncated display text.
    let (start, end) = sig
        .params
        .get(editor.signature.active_parameter)
        .copied()
        .map(|(s, e)| {
            (
                s.min(label.len()),
                e.min(label.len()).max(s.min(label.len())),
            )
        })
        .unwrap_or((label.len(), label.len()));
    let pre = &label[..start];
    let param = &label[start..end];
    let post = &label[end..];
    let mut col = x + 1;
    for (text, style) in [(pre, base), (param, highlight), (post, base)] {
        if text.is_empty() {
            continue;
        }
        surface.set_text(col, row, text, Some(style));
        col += text.chars().count() as u16;
    }
}
