//! Drawing helpers for icon-pack entries — glyphs from the bundled icon
//! font and rasterized SVGs from VS Code icon-theme packs.  SVG textures
//! are cached per (path, pixel size) in egui's context memory.

use std::path::Path;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Vec2};

use crate::kernel::icons::IconKind;

/// Square box an icon is drawn in; callers advance x by this much.
pub const SLOT: f32 = 16.0;

/// Draw an icon centered at `center`, clipped to a `slot`-wide box.
/// `color` is the fallback tint for glyphs the pack leaves uncolored.
pub fn draw(
    ui: &mut egui::Ui,
    kind: &IconKind,
    center: Pos2,
    slot: f32,
    font_id: FontId,
    color: Color32,
) {
    match kind {
        IconKind::Glyph { glyph, color: tint } => {
            ui.painter().text(
                center,
                Align2::CENTER_CENTER,
                glyph.to_string(),
                font_id,
                tint.map(|(r, g, b)| Color32::from_rgb(r, g, b)).unwrap_or(color),
            );
        }
        IconKind::Svg { path } => {
            let scale = ui.ctx().pixels_per_point();
            let px = ((slot * scale) as usize).max(8);
            let Some(texture) = svg_texture(ui.ctx(), path, px) else {
                return;
            };
            let rect = egui::Rect::from_center_size(center, Vec2::splat(slot));
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}

fn svg_texture(ctx: &egui::Context, path: &Path, px: usize) -> Option<egui::TextureHandle> {
    let key = egui::Id::new(("magma-icon-svg", path.to_string_lossy().to_string(), px));
    let cached: Option<egui::TextureHandle> = ctx.data(|d| d.get_temp(key));
    if let Some(t) = cached {
        return Some(t);
    }
    let bytes = std::fs::read(path).ok()?;
    let texture = rasterize_svg(ctx, &bytes, px)?;
    ctx.data_mut(|d| d.insert_temp(key, texture.clone()));
    Some(texture)
}

fn rasterize_svg(ctx: &egui::Context, bytes: &[u8], px: usize) -> Option<egui::TextureHandle> {
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px as u32, px as u32)?;
    let scale = (px as f32) / size.width().max(1.0);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia is premultiplied; egui expects straight alpha.
    let mut pixels = Vec::with_capacity(px * px * 4);
    for chunk in pixmap.data().chunks_exact(4) {
        let [r, g, b, a] = [chunk[0], chunk[1], chunk[2], chunk[3]];
        if a == 0 {
            pixels.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32) as u8;
            pixels.extend_from_slice(&[un(r), un(g), un(b), a]);
        }
    }
    Some(ctx.load_texture(
        format!("magma-icon-{}", px),
        egui::ColorImage::from_rgba_unmultiplied([px, px], &pixels),
        egui::TextureOptions::LINEAR,
    ))
}
