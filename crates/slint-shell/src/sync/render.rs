//! Pane and gutter rasterization: kernel Surface → Slint images.
//!
//! Re-blits only when a signature changes, so status, tabs, and terminal
//! output updates do not touch the editor pixels.

use kernel::render::surface::{Style, Surface};

use crate::{AppState, AppWindow, pane};

pub(crate) fn sync_pane_images(app: &AppWindow, state: &mut AppState, ppx: f32) {
    let ed = &mut state.editor;
    let focused_key = ed.focused_buffer_key();
    let is_image = ed
        .buffers
        .get(focused_key)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).image.is_some())
        .unwrap_or(false);

    // Only re-blit and re-upload when the grid actually changed: status,
    // tabs, terminal output and other chrome updates do not touch it.
    // Image buffers render the decoded image instead of the text grid.
    let focused_key = ed.focused_buffer_key();
    let is_image = ed
        .buffers
        .get(focused_key)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).image.is_some())
        .unwrap_or(false);

    if is_image {
        let sig = pane::surface_signature(&state.surface);
        let img = ed
            .buffers
            .get(focused_key)
            .unwrap()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .image
            .clone()
            .unwrap();
        let key = focused_key as u64 * 1_000_003 + img.width as u64 * 31 + img.height as u64;
        if key != state.image_pane_key {
            let bg = ed.theme_color("bg");
            let pw = state.surface.width as u32
                * pane::cell_size(state.cell_w, state.cell_h, state.atlas_ppx).0;
            let ph = state.surface.height as u32
                * pane::cell_size(state.cell_w, state.cell_h, state.atlas_ppx).1;
            let (rgba, w, h) = pane::blit_image(&img, pw.max(1), ph.max(1), bg);
            let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w.max(1), h.max(1));
            let dst = buffer.make_mut_bytes();
            let n = dst.len().min(rgba.len());
            dst[..n].copy_from_slice(&rgba[..n]);
            app.set_pane(slint::Image::from_rgba8(buffer));
            state.image_pane_key = key;
        }
        // No gutter overlay for image buffers.
        app.set_gutter_visible(false);
        app.set_gutter(slint::Image::default());
        state.surface_sig = sig;
        state.gutter_sig = 0;
    } else {
        // ── Gutter: render the fixed left columns as a native overlay ──
        // The kernel still paints the gutter into the surface (so mouse
        // hit-testing and overlay math keep working), but we lift those
        // columns into a separate image and blank them in the pane so the
        // gutter is not drawn twice.
        let has_buf = ed.buffers.get(focused_key).is_some();
        let total_lines = ed
            .buffers
            .get(focused_key)
            .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
            .unwrap_or(1);
        let gutter_cols = if !has_buf {
            0
        } else {
            let gc = kernel::render::frame::gutter_cols(&*ed, focused_key, total_lines);
            gc
        };

        // Build a sub-surface holding only the gutter columns.
        let gutter_surf = if gutter_cols > 0 {
            let mut gs = Surface::new(gutter_cols as u16, state.surface.height);
            for y in 0..state.surface.height {
                for x in 0..gutter_cols as u16 {
                    if let Some(cell) = state.surface.cell(x, y) {
                        gs.set_cell(x, y, cell.ch, Some(cell.style));
                    }
                }
            }
            Some(gs)
        } else {
            None
        };

        let gutter_sig = gutter_surf
            .as_ref()
            .map(|gs| pane::surface_signature(gs))
            .unwrap_or(0);

        if gutter_cols > 0 && gutter_sig != state.gutter_sig {
            if let Some(gs) = &gutter_surf {
                let (rgba, pw, ph) = pane::blit_rgba(
                    gs,
                    &state.atlas,
                    state.cell_w,
                    state.cell_h,
                    state.atlas_ppx,
                );
                let mut buffer =
                    slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(pw.max(1), ph.max(1));
                let dst = buffer.make_mut_bytes();
                let n = dst.len().min(rgba.len());
                dst[..n].copy_from_slice(&rgba[..n]);
                app.set_gutter(slint::Image::from_rgba8(buffer));
            }
            state.gutter_sig = gutter_sig;
        } else if gutter_cols == 0 {
            app.set_gutter(slint::Image::default());
            state.gutter_sig = 0;
        }

        // Blank the gutter columns in the main surface so they are not
        // painted twice (the overlay above covers this region).
        if gutter_cols > 0 {
            let (bg_r, bg_g, bg_b) = ed.theme_color("bg");
            for y in 0..state.surface.height {
                for x in 0..gutter_cols as u16 {
                    state.surface.set_cell(
                        x,
                        y,
                        ' ',
                        Some(Style {
                            fg: (bg_r, bg_g, bg_b),
                            bg: (bg_r, bg_g, bg_b),
                            ..Default::default()
                        }),
                    );
                }
            }
        }

        app.set_gutter_visible(gutter_cols > 0);
        app.set_gutter_width((gutter_cols as f32) * (state.cell_w as f32));

        // Now compute pane signature (after blanking) and re-blit if changed.
        let sig = pane::surface_signature(&state.surface);
        if sig != state.surface_sig {
            let (rgba, pw, ph) = pane::blit_rgba(
                &state.surface,
                &state.atlas,
                state.cell_w,
                state.cell_h,
                state.atlas_ppx,
            );
            let mut buffer =
                slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(pw.max(1), ph.max(1));
            let dst = buffer.make_mut_bytes();
            let n = dst.len().min(rgba.len());
            dst[..n].copy_from_slice(&rgba[..n]);
            app.set_pane(slint::Image::from_rgba8(buffer));
            state.surface_sig = sig;
        }
    }
}
