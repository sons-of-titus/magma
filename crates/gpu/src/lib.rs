//! GPU-side text rendering: the glyph atlas, glyph rasterization, and the
//! wgpu paint callback that uploads the atlas and draws the editor's quads.
//!
//! The atlas module is CPU-only — it holds UVs, instances, and pixels, and
//! is testable without a device.  Rasterization and the paint callback are
//! behind the `gui` feature because they need `ab_glyph` and `wgpu`.

#[cfg(test)]
mod tests;

pub mod gpu_atlas;

#[cfg(feature = "gui")]
pub mod gpu_rasterizer;

#[cfg(feature = "gui")]
pub mod gpu_paint_callback;

pub use gpu_atlas::{
    ATLAS_CELL_H, ATLAS_CELL_W, ATLAS_CHARS, ATLAS_COLS, GlyphInstance, GlyphUv, GpuGlyphAtlas,
    RectInstance, build_glyph_instances, build_rect_instances, glyphs_to_vertices,
    rects_to_vertices,
};

#[cfg(feature = "gui")]
pub use gpu_paint_callback::MagmaPaintCallback;
