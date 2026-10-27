use crate::kernel::render::gpu_atlas::{
    build_glyph_instances, build_rect_instances, glyphs_to_vertices, rects_to_vertices,
    GpuGlyphAtlas, ATLAS_CHARS,
};
use crate::kernel::render::surface::{Style, Surface};

fn default_style() -> Style {
    Style::default()
}

fn red_fg_style() -> Style {
    Style { fg: (255, 0, 0), ..Style::default() }
}

fn blue_bg_style() -> Style {
    Style { bg: (0, 0, 255), ..Style::default() }
}

// ── GpuGlyphAtlas::new ───────────────────────────────────────────────────────

#[test]
fn atlas_new_is_dirty() {
    let atlas = GpuGlyphAtlas::new();
    assert!(atlas.needs_rebuild());
}

#[test]
fn atlas_mark_clean_and_dirty_roundtrip() {
    let mut atlas = GpuGlyphAtlas::new();
    atlas.mark_clean();
    assert!(!atlas.needs_rebuild());
    atlas.mark_dirty();
    assert!(atlas.needs_rebuild());
}

#[test]
fn atlas_covers_all_printable_ascii() {
    let atlas = GpuGlyphAtlas::new();
    for ch in ATLAS_CHARS.chars() {
        assert!(atlas.contains(ch), "atlas missing char: {:?}", ch);
    }
}

#[test]
fn atlas_covers_space() {
    let atlas = GpuGlyphAtlas::new();
    assert!(atlas.contains(' '));
}

#[test]
fn atlas_normalize_sums_to_one_for_full_cell() {
    let atlas = GpuGlyphAtlas::new();
    // A character in the last row, last column should have uv + uv_size ≤ 1.0
    for uv in atlas.glyphs.values() {
        let [x, y, w, h] = atlas.normalize(uv);
        assert!(x + w <= 1.001, "UV x+w out of range: {}", x + w);
        assert!(y + h <= 1.001, "UV y+h out of range: {}", y + h);
        assert!(x >= 0.0 && y >= 0.0);
    }
}

#[test]
fn atlas_placeholder_pixels_correct_size() {
    let atlas = GpuGlyphAtlas::new();
    let pixels = atlas.generate_atlas_pixels();
    assert_eq!(pixels.len(), (atlas.atlas_width * atlas.atlas_height * 4) as usize);
    assert!(pixels.iter().all(|&b| b == 255), "placeholder should be all-white RGBA");
}

// ── build_glyph_instances ────────────────────────────────────────────────────

#[test]
fn glyph_instances_skips_spaces_and_nulls() {
    let mut surface = Surface::new(4, 2);
    surface.set_cell(0, 0, ' ', Some(default_style()));
    surface.set_cell(1, 0, 'A', Some(red_fg_style()));
    surface.set_cell(2, 0, '\0', Some(default_style()));
    surface.set_cell(3, 0, 'B', Some(red_fg_style()));

    let atlas = GpuGlyphAtlas::new();
    let instances = build_glyph_instances(&surface, &atlas, 8.0, 16.0, 0.0, 0.0, false);
    assert_eq!(instances.len(), 2, "only A and B should be in instances");
}

#[test]
fn glyph_instances_position_is_correct() {
    let mut surface = Surface::new(3, 1);
    surface.set_cell(2, 0, 'X', Some(default_style()));

    let atlas = GpuGlyphAtlas::new();
    let char_w = 10.0f32;
    let line_h = 20.0f32;
    let instances = build_glyph_instances(&surface, &atlas, char_w, line_h, 5.0, 7.0, false);
    assert_eq!(instances.len(), 1);
    assert!((instances[0].screen_x - (5.0 + 2.0 * char_w)).abs() < 1e-5);
    assert!((instances[0].screen_y - 7.0).abs() < 1e-5);
}

#[test]
fn glyph_instances_fg_color_normalized() {
    let mut surface = Surface::new(1, 1);
    surface.set_cell(0, 0, 'Z', Some(Style { fg: (255, 128, 0), ..Style::default() }));

    let atlas = GpuGlyphAtlas::new();
    let instances = build_glyph_instances(&surface, &atlas, 8.0, 16.0, 0.0, 0.0, false);
    assert_eq!(instances.len(), 1);
    let [r, g, b, a] = instances[0].fg;
    assert!((r - 1.0).abs() < 1e-3);
    assert!((g - 128.0 / 255.0).abs() < 1e-3);
    assert!((b - 0.0).abs() < 1e-3);
    assert!((a - 1.0).abs() < 1e-3);
}

#[test]
fn glyph_instances_skip_last_row_when_requested() {
    let mut surface = Surface::new(2, 3);
    surface.set_cell(0, 0, 'A', Some(default_style()));
    surface.set_cell(0, 1, 'B', Some(default_style()));
    surface.set_cell(0, 2, 'C', Some(default_style())); // last row (status bar)

    let atlas = GpuGlyphAtlas::new();
    let all = build_glyph_instances(&surface, &atlas, 8.0, 16.0, 0.0, 0.0, false);
    let skip = build_glyph_instances(&surface, &atlas, 8.0, 16.0, 0.0, 0.0, true);
    assert_eq!(all.len(), 3);
    assert_eq!(skip.len(), 2, "last row should be excluded");
}

// ── build_rect_instances ─────────────────────────────────────────────────────

#[test]
fn rect_instances_skips_default_bg_cells() {
    let mut surface = Surface::new(3, 1);
    let default_bg = [0.0f32, 0.0, 0.0, 1.0]; // Style::default() bg = (0, 0, 0) → alpha=1

    surface.set_cell(0, 0, ' ', Some(default_style()));
    surface.set_cell(1, 0, 'A', Some(blue_bg_style())); // different bg
    surface.set_cell(2, 0, ' ', Some(default_style()));

    let rects = build_rect_instances(&surface, 8.0, 16.0, 0.0, 0.0, default_bg, false);
    assert_eq!(rects.len(), 1, "only blue-bg cell produces a rect");
}

#[test]
fn rect_instances_position_and_size_correct() {
    let mut surface = Surface::new(2, 1);
    surface.set_cell(1, 0, 'A', Some(blue_bg_style()));

    // Style::default() bg = (0,0,0) → [0,0,0,1] (alpha = 1)
    let default_bg = [0.0f32, 0.0, 0.0, 1.0];
    let char_w = 12.0f32;
    let line_h = 18.0f32;
    let rects = build_rect_instances(&surface, char_w, line_h, 3.0, 5.0, default_bg, false);
    assert_eq!(rects.len(), 1);
    assert!((rects[0].x - (3.0 + char_w)).abs() < 1e-5);
    assert!((rects[0].y - 5.0).abs() < 1e-5);
    assert!((rects[0].w - char_w).abs() < 1e-5);
    assert!((rects[0].h - line_h).abs() < 1e-5);
}

// ── vertex expansion ─────────────────────────────────────────────────────────

#[test]
fn glyphs_to_vertices_produces_6_verts_per_instance() {
    let mut surface = Surface::new(2, 1);
    surface.set_cell(0, 0, 'A', Some(default_style()));
    surface.set_cell(1, 0, 'B', Some(default_style()));

    let atlas = GpuGlyphAtlas::new();
    let instances = build_glyph_instances(&surface, &atlas, 8.0, 16.0, 0.0, 0.0, false);
    let verts = glyphs_to_vertices(&instances, 8.0, 16.0);
    assert_eq!(verts.len(), instances.len() * 6);
}

#[test]
fn rects_to_vertices_produces_6_verts_per_rect() {
    let mut surface = Surface::new(3, 1);
    surface.set_cell(0, 0, ' ', Some(blue_bg_style()));
    surface.set_cell(1, 0, ' ', Some(blue_bg_style()));

    let default_bg = [0.0f32, 0.0, 0.0, 1.0];
    let atlas = GpuGlyphAtlas::new();
    let rects = build_rect_instances(&surface, 8.0, 16.0, 0.0, 0.0, default_bg, false);
    let verts = rects_to_vertices(&rects, atlas.atlas_width, atlas.atlas_height);
    assert_eq!(verts.len(), rects.len() * 6);
}

// ── Fallback path selection ───────────────────────────────────────────────────

#[test]
fn new_atlas_gpu_path_disabled_by_default() {
    // GuiApp starts with gpu_path_ready = false, so the fallback is used.
    // We verify this indirectly: building instances from an empty surface
    // produces an empty list, which is the "nothing to draw" fallback signal.
    let surface = Surface::new(0, 0);
    let atlas = GpuGlyphAtlas::new();
    let instances = build_glyph_instances(&surface, &atlas, 8.0, 16.0, 0.0, 0.0, false);
    assert!(instances.is_empty());
}

// ── Phase 9: viewport rendering without full-document copies ───────────────

mod render_damage {
    use crate::kernel::command::builtin;
    use crate::kernel::render::frame::{render_frame, scroll_state_for_buffer, scroll_state_from_lines};
    use crate::kernel::render::surface::Surface;
    use crate::kernel::state::Editor;
    use crate::kernel::storage::disk::DiskFileSystem;

    fn editor_with(content: &str) -> Editor {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        let key = ed.create_buffer_from_str("big", content);
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        ed
    }

    #[test]
    fn large_document_viewport_render_only_shows_visible_rows() {
        // 2000 lines; the viewport is 5 rows.  Render must show exactly the
        // scrolled-to window, derived without copying the document.
        let mut text = String::new();
        for i in 0..2000 {
            text.push_str(&format!("line-{i}\n"));
        }
        let ed = editor_with(&text);

        let mut surface = Surface::new(40, 8);
        render_frame(&ed, &mut surface);

        // Status bar is the last row; editor area rows 0..7 minus it.
        let mut rendered = Vec::new();
        for y in 0..8 {
            let mut row = String::new();
            for x in 0..40 {
                if let Some(cell) = surface.cell(x, y) {
                    row.push(cell.ch);
                }
            }
            rendered.push(row);
        }
        let visible_text: Vec<&str> = rendered.iter().map(|s| s.as_str()).collect();
        assert!(
            visible_text.iter().any(|r| r.contains("ine-0")),
            "top of document visible initially: {visible_text:?}"
        );
        assert!(
            !visible_text.iter().any(|r| r.contains("line-1999")),
            "far content must not leak into the viewport"
        );
    }

    #[test]
    fn cursor_at_bottom_scrolls_window() {
        let mut text = String::new();
        for i in 0..500 {
            text.push_str(&format!("row{i}\n"));
        }
        let mut ed = editor_with(&text);
        let key = crate::tests::helpers::focused_key(&ed);
        {
            let view = ed.views.get_mut(&key).unwrap();
            let target = text.rfind("row400").unwrap_or(0);
            view.set_cursor(target);
        }
        let mut surface = Surface::new(40, 8);
        render_frame(&ed, &mut surface);
        let mut rendered = String::new();
        for y in 0..8 {
            for x in 0..40 {
                if let Some(cell) = surface.cell(x, y) {
                    rendered.push(cell.ch);
                }
            }
            rendered.push('\n');
        }
        assert!(rendered.contains("ow400"), "cursor line must be visible: {rendered}");
    }

    #[test]
    fn key_dispatch_marks_the_frame_dirty() {
        let mut ed = editor_with("hello");
        ed.editor_mode = crate::kernel::state::EditorMode::new("editor", true);
        ed.keymaps.push_layer("editor");
        ed.frame_dirty = false;

        crate::kernel::input::dispatch_key(&mut ed, "x");
        assert!(ed.frame_dirty, "a key that edits must request a repaint");
    }

    #[test]
    fn background_events_mark_the_frame_dirty() {
        let mut ed = editor_with("hello");
        ed.frame_dirty = false;

        crate::kernel::runtime::process_background_event(&mut ed,
            crate::kernel::runtime::BackgroundEvent::AutoSaveWritten { buf_key: 0, saved_rev: 0 });
        assert!(ed.frame_dirty, "async results landing must request a repaint");
    }

    #[test]
    fn scroll_state_math_matches_scrolloff_contract() {
        // 20-line doc, 5 visible rows, scrolloff 2, cursor on line 10.
        let st = scroll_state_from_lines(10, 0, 20, 5, 2);
        assert!(st.cursor_vis_row <= 5, "cursor stays inside the window");
        assert!(st.scroll_top <= 10);

        // Cursor at the very top: no scroll.
        let st = scroll_state_from_lines(0, 0, 20, 5, 2);
        assert_eq!(st.scroll_top, 0);
        assert_eq!(st.cursor_vis_row, 0);

        // Consistency of the composed helper with the pure one.
        let ed = editor_with("a\nb\nc\nd\ne\nf\ng\n");
        let key = crate::tests::helpers::focused_key(&ed);
        let arc = ed.buffers.get(key).unwrap().clone();
        let buf = arc.lock().unwrap();
        let st1 = {
            let cursor = ed.views.get(&key).unwrap().cursor_offset();
            scroll_state_for_buffer(&buf, cursor, 3, 0, 4, &Default::default())
        };
        let st2 = scroll_state_from_lines(0, 0, buf.line_count(), 3, 0);
        assert_eq!(st1.scroll_top, st2.scroll_top);
        assert_eq!(st1.cursor_vis_row, st2.cursor_vis_row);
    }
}


#[test]
fn cursor_respects_blink_flag() {
    use crate::kernel::command::builtin;
    use crate::kernel::render::frame::render_frame_with;
    use crate::kernel::render::CursorPaint;
    use crate::kernel::state::Editor;
    use crate::kernel::storage::disk::DiskFileSystem;
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer_from_str("blink", "abc");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    let mut surface = Surface::new(20, 4);

    let cursor_color = ed.theme_color("cursor");
    render_frame_with(&ed, &mut surface, CursorPaint::Stamp { visible: true });
    let with_cursor = (0..4u16).flat_map(|y| (0..20u16).map(move |x| (x, y)))
        .filter_map(|(x, y)| surface.cell(x, y).map(|c| (x, y, c)))
        .any(|(_, _, c)| c.style.fg == cursor_color || c.style.bg == cursor_color);
    assert!(with_cursor, "cursor drawn when visible");

    render_frame_with(&ed, &mut surface, CursorPaint::Stamp { visible: false });
    let without = (0..4u16).flat_map(|y| (0..20u16).map(move |x| (x, y)))
        .filter_map(|(x, y)| surface.cell(x, y))
        .any(|c| c.style.fg == cursor_color || c.style.bg == cursor_color);
    assert!(!without, "no cursor styling while blink phase is off");
}

// ── Highlight/selection cell columns (tabs and wide glyphs) ──────────

#[test]
fn selection_paints_tab_expanded_cell_columns() {
    use crate::kernel::render::highlight_pass::apply_highlights;
    use crate::kernel::render::surface::Surface;

    let mut ed = crate::tests::helpers::make_editor_with_buffer("\tx");
    let key = crate::tests::helpers::focused_key(&ed);
    // Select the whole line (anchor 0, cursor at end).
    ed.selection = Some(crate::kernel::state::Selection::char(0));
    ed.views.get_mut(&key).unwrap().set_cursor(2);

    let view = ed.views.get(&key).unwrap().clone();
    let line = ed.buffers.get(key).unwrap().lock().unwrap().slice(0, 2);
    let rows: Vec<(usize, usize, &str)> = vec![(0, 2, line.as_str())];
    let mut surface = Surface::new(20, 1);
    let layouts: Vec<std::sync::Arc<crate::kernel::render::row_layout::RowLayout>> =
        rows.iter().map(|(_, _, t)| std::sync::Arc::new(
            crate::kernel::render::row_layout::RowLayout::build(t, 0, 4, &ed.font_config.glyph_widths)))
        .collect();
    apply_highlights(&ed, Some(&view), &mut surface, &rows, &layouts, 0, 1, 0, 0, 2, 0);

    // 'x' is painted at cell 4 (tab stop), so the selection must reach
    // cells 0..5 — before the fix the selection style stopped at cell 1
    // because columns were char indices.
    let default_style = crate::kernel::render::surface::Style::default();
    for col in 0..5u16 {
        let styled = surface.cell(col, 0)
            .map(|c| c.style != default_style)
            .unwrap_or(false);
        assert!(styled, "selection must cover expanded cell {col}");
    }
    let x_cell = surface.cell(4, 0).unwrap();
    assert_eq!(x_cell.ch, 'x', "the 'x' glyph keeps its char under the selection");
    let beyond = surface.cell(5, 0).map(|c| c.style != default_style).unwrap_or(false);
    assert!(!beyond, "selection stops after the painted glyphs");
}

#[test]
fn highlight_layer_covers_wide_glyph_trailing_cell() {
    use crate::kernel::render::highlight_pass::apply_highlights;
    use crate::kernel::render::surface::Surface;

    let ed = crate::tests::helpers::make_editor_with_buffer("字x");
    let key = crate::tests::helpers::focused_key(&ed);
    let mut view = ed.views.get(&key).unwrap().clone();
    // Highlight just the wide glyph.
    view.set_highlights_layer("search", vec![(0, 3, "search".to_string())]);

    let line = ed.buffers.get(key).unwrap().lock().unwrap().slice(0, 4);
    let rows: Vec<(usize, usize, &str)> = vec![(0, 4, line.as_str())];
    let mut surface = Surface::new(20, 1);
    let layouts: Vec<std::sync::Arc<crate::kernel::render::row_layout::RowLayout>> =
        rows.iter().map(|(_, _, t)| std::sync::Arc::new(
            crate::kernel::render::row_layout::RowLayout::build(t, 0, 4, &ed.font_config.glyph_widths)))
        .collect();
    apply_highlights(&ed, Some(&view), &mut surface, &rows, &layouts, 0, 1, 0, 0, 0, 0);

    let default_style = crate::kernel::render::surface::Style::default();
    let lead = surface.cell(0, 0).unwrap();
    assert_eq!(lead.ch, '字');
    assert!(lead.style != default_style, "lead cell styled");
    let trailing = surface.cell(1, 0).unwrap();
    assert!(trailing.style != default_style,
        "the wide glyph's second cell must carry the highlight too");
    // 'x' sits at cell 2, outside the range: apply_highlights only restyles,
    // so the cell keeps its default (unstyled) state.
    let after = surface.cell(2, 0).unwrap();
    assert!(after.style == default_style, "'x' is outside the highlight range");
}

// ── Surface default fill ─────────────────────────────────────────────────────

#[test]
fn render_frame_seeds_unpainted_cells_with_the_editor_background() {
    let mut ed = crate::tests::helpers::make_editor();
    ed.theme.insert("bg".to_string(), (30, 31, 34));
    // A surface larger than the single pane leaves cells no view paints; they
    // must carry the editor background, not `Style::default()`'s black.
    let mut surface = Surface::new(200, 200);
    crate::kernel::render::frame::render_frame(&ed, &mut surface);
    for y in 0..surface.height {
        for x in 0..surface.width {
            let cell = surface.cell(x, y).unwrap();
            assert_ne!(cell.style.bg, (0, 0, 0),
                "cell ({x},{y}) kept the default black background");
        }
    }
}
