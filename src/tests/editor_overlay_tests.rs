//! EditorView overlay-policy and decoration-geometry regressions.
//!
//! Split from `render_tests.rs` (atlas/atmosphere) to keep both files
//! under the size gate.  Covers: the native completion popup staying out
//! of the pane texture, blame margin geometry, fold-aware decoration
//! rows, and the line-selection span.

use crate::kernel::render::surface::Surface;

// ── Overlay cursor policy: the native completion popup ──────────────────────

fn popup_probe_editor() -> crate::kernel::state::Editor {
    use crate::kernel::command::builtin;
    use crate::kernel::state::Editor;
    use crate::kernel::storage::disk::DiskFileSystem;
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer_from_str("comp", "ab\ncd\nef\ngh\nij\nkl\nmn\n");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed.completion.visible = true;
    ed.completion.items = vec![
        crate::kernel::state::completion::CompletionItem::word(String::from("alpha")),
        crate::kernel::state::completion::CompletionItem::word(String::from("beta")),
    ];
    ed
}

/// The in-grid popup stamps a (40, 40, 40) background over the popup area;
/// no pane cell legitimately carries that color, so any hit means stamped.
fn popup_stamped(surface: &crate::kernel::render::surface::Surface) -> bool {
    let mut hit = false;
    for y in 0..surface.height {
        for x in 0..surface.width {
            if surface.cell(x, y).map(|c| c.style.bg == (40, 40, 40)) == Some(true) {
                hit = true;
            }
        }
    }
    hit
}

#[test]
fn overlay_cursor_policy_leaves_the_completion_popup_out_of_the_grid() {
    // The Slint shell draws the popup as a native overlay; baking it into
    // the pane texture as well rendered it twice.
    use crate::kernel::render::frame::render_frame_with;
    use crate::kernel::render::CursorPaint;
    let ed = popup_probe_editor();
    let mut surface = crate::kernel::render::surface::Surface::new(20, 8);
    // Pane defaults are 80x24; the popup stamps its last visible rows, so
    // the surface must cover the whole pane for the probe to see it.
    let mut surface = crate::kernel::render::surface::Surface::new(80, 24);
    render_frame_with(&ed, &mut surface, CursorPaint::Overlay);
    assert!(
        !popup_stamped(&surface),
        "Overlay must not bake the popup into the texture"
    );

    render_frame_with(&ed, &mut surface, CursorPaint::Stamp { visible: true });
    assert!(
        popup_stamped(&surface),
        "the headless Stamp policy still owns the popup"
    );
}

#[test]
fn overlay_cursor_policy_still_stamps_the_caret() {
    use crate::kernel::render::frame::render_frame_with;
    use crate::kernel::render::CursorPaint;
    let mut ed = popup_probe_editor();
    ed.completion.visible = false;
    ed.completion.items.clear();
    let cursor_color = ed.theme_color("cursor");
    let mut surface = crate::kernel::render::surface::Surface::new(20, 8);
    render_frame_with(&ed, &mut surface, CursorPaint::Overlay);
    let caret = (0..8u16).any(|y| {
        (0..20u16).any(|x| {
            surface
                .cell(x, y)
                .map(|c| c.style.fg == cursor_color || c.style.bg == cursor_color)
                .unwrap_or(false)
        })
    });
    assert!(caret, "Overlay stamps the caret; only the popup is native");
}

// ── Decoration geometry ──────────────────────────────────────────────────────

#[test]
fn blame_prefix_decorations_render_in_the_margin_left_of_the_gutter() {
    // The regression: the prefix was drawn at surface column gutter_width —
    // on top of the text — instead of the reserved margin at area.x.
    use crate::kernel::render::frame::render_frame_with;
    use crate::kernel::render::CursorPaint;
    let mut ed = crate::tests::helpers::make_editor();
    let key = ed.create_buffer_from_str("blamed", "one\ntwo\nthree\n");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed.view_of_buffer_mut(key)
        .unwrap()
        .decor_set_prefix("vc-blame", 1, String::from("Alice 2026-10-25"), "comment-face".to_string());

    let mut surface = crate::kernel::render::surface::Surface::new(40, 6);
    render_frame_with(&ed, &mut surface, CursorPaint::Overlay);

    // "Alice" starts at column 0 of the line's row.
    let cell = surface.cell(0, 1).unwrap();
    assert_eq!(cell.ch, 'A', "prefix drawn in the margin at area.x");
    // The line's own text is intact one gutter past the margin
    // (prefix_margin 16 chars + 1 separator ⇒ content at column 17).
    let text = surface.cell(17, 1).unwrap();
    assert_eq!(text.ch, 't', "the margin reservation leaves the text intact");
}

#[test]
fn decorations_track_fold_compression() {
    use crate::kernel::render::frame::render_frame_with;
    use crate::kernel::render::CursorPaint;
    let mut ed = crate::tests::helpers::make_editor();
    //           0   4   8   14  19
    let key = ed.create_buffer_from_str("folded", "one\ntwo\nthree\nfour\nfive\n");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    // Fold lines 1..=2 (offsets 4..14): "four" moves from display row 3 to 2.
    ed.view_of_buffer_mut(key).unwrap().add_fold(4, 14);
    ed.view_of_buffer_mut(key)
        .unwrap()
        .decor_set_prefix("vc-blame", 3, String::from("Bob"), "comment-face".to_string());

    let mut surface = crate::kernel::render::surface::Surface::new(40, 6);
    render_frame_with(&ed, &mut surface, CursorPaint::Overlay);

    assert_eq!(surface.cell(0, 2).unwrap().ch, 'B', "prefix follows the folded row");
    assert_ne!(
        surface.cell(0, 3).unwrap().ch,
        'B',
        "the unfolded row position must stay empty"
    );
}

// ── Line-selection span ──────────────────────────────────────────────────────

#[test]
fn linewise_selection_matches_the_brute_force_span() {
    // The old computation scanned the whole document per frame; the line
    // index version must produce the same span.
    use crate::kernel::state::mode::Selection;
    let mut ed = crate::tests::helpers::make_editor();
    let body: String = (0..200).map(|i| format!("line {i}\n")).collect();
    let key = ed.create_buffer_from_str("big", &body);
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    let arc = ed.buffers.get(key).unwrap();
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    let line_start = |l: usize| buf.line_start_offset(l).unwrap_or(buf.len());

    for cursor_line in [0usize, 1, 57, 150, 199] {
        let cursor = line_start(cursor_line);
        for anchor_line in [0usize, 57, 199] {
            ed.selection = Some(Selection::line(line_start(anchor_line)));
            let got = ed.selection_range(cursor).unwrap();
            let first = anchor_line.min(cursor_line);
            let last = anchor_line.max(cursor_line);
            let want = (line_start(first), line_start(last + 1));
            assert_eq!(got, want, "lines {anchor_line}..{cursor_line}");
        }
    }
}
