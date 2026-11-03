//! CPU-side text rendering: the glyph atlas, its UV layout, and the glyph
//! and rectangle instance builders.  No wgpu types live here, so the atlas
//! is testable without a device; the GPU pipeline belongs to the frontend.

#[cfg(test)]
mod tests;

pub mod gpu_atlas;
pub mod gpu_rasterizer;
pub mod oblique;
pub mod shapes;

pub use gpu_atlas::{
    ATLAS_CELL_H, ATLAS_CELL_W, ATLAS_CHARS, ATLAS_COLS, GlyphInstance, GlyphUv, GpuGlyphAtlas,
    RectInstance, build_glyph_instances, build_rect_instances, glyphs_to_vertices,
    rects_to_vertices,
};
