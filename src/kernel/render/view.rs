//! View trait — the renderable unit in the ViewTree.

use crate::kernel::render::surface::Surface;
use crate::kernel::state::Editor;

/// Axis-aligned rectangle in surface-absolute coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Rect { x, y, width, height }
    }
}

/// Context passed into every View::render call.
pub struct RenderCtx<'a> {
    pub editor: &'a Editor,
    /// Surface-absolute area this view occupies.
    pub area: Rect,
}

/// What kind of content a view produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    Editor,
    Sidebar,
    Terminal,
}

/// Core rendering trait — a view paints itself into the surface region given by ctx.area.
pub trait View {
    fn render(&self, surface: &mut Surface, ctx: &RenderCtx);
    fn kind(&self) -> ViewKind;
}
