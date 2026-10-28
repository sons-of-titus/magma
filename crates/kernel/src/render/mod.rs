//! Rendering — the `Surface` character-grid pipeline the frontend paints from.

pub use surface;
pub mod frame;
pub mod host;
pub use host::{FrameHost, PaneMetrics, RenderHost, ViewportHost};
pub mod view;
pub use view_tree;
pub mod caret_extras;
pub mod editor_view;
pub mod gutter;
pub mod gutter_providers;
pub mod highlight_pass;
pub use gpu::{gpu_atlas, shapes};
pub use row_layout;
pub mod decorations;
pub mod tabs;

pub use view::{CursorPaint, RenderCtx};
pub use view_tree::{LayoutConstraint, Pane, SplitDirection, ViewTree};
