//! Rendering abstraction — `Surface`-based pipeline shared by TUI and GUI backends.

pub mod surface;
pub mod tui;
pub mod frame;
pub mod view;
pub mod view_tree;
pub mod editor_view;
pub mod terminal_view;
pub mod sidebar_view;
pub mod gutter;
pub mod gutter_providers;
pub(crate) mod status_and_popup;
pub(crate) mod highlight_pass;
pub mod gpu_atlas;
pub mod decorations;
#[cfg(feature = "gui")]
pub mod gpu_paint_callback;
#[cfg(feature = "gui")]
pub mod gui;
#[cfg(feature = "gui")]
pub(crate) mod gui_fonts;
#[cfg(feature = "gui")]
pub(crate) mod gui_render;

pub use view_tree::{ViewTree, Pane, LayoutConstraint, SplitDirection};

use surface::Surface;
use crate::kernel::input::event::InputEvent;

pub trait RenderTrait: Send {
    fn draw(&mut self, surface: &Surface);
    fn poll_event(&mut self) -> Option<InputEvent>;
    fn dimensions(&self) -> (u16, u16);
    fn set_title(&mut self, title: &str);
    fn close(&mut self);
}
