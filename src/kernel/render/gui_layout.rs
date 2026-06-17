//! GUI layout state — panel visibility and sizing.

#[cfg(feature = "gui")]
pub struct GuiLayout {
    pub sidebar_open: bool,
    pub sidebar_width: f32,
}

#[cfg(feature = "gui")]
impl Default for GuiLayout {
    fn default() -> Self {
        Self { sidebar_open: true, sidebar_width: 220.0 }
    }
}
