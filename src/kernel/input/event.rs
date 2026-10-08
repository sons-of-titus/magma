//! Input event types produced by the GUI adapter and consumed by
//! `input::mouse`.

/// A mouse event in cell coordinates, produced by a renderer backend.
#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub x: u16,
    pub y: u16,
    pub button: MouseButton,
    /// Comma-separated modifier names: "shift", "ctrl", "alt".
    /// Populated by the renderer backend from the platform event.
    pub modifiers: String,
}

/// The kind of mouse action.
#[derive(Debug, Clone, PartialEq)]
pub enum MouseEventKind {
    /// Button pressed (click).
    Click,
    /// Mouse moved while a button is held (drag).
    Drag,
    /// Scroll wheel moved.  Positive delta = scroll down.
    Scroll(i32),
    /// Button released.
    Release,
}

/// Which mouse button was involved.
#[derive(Debug, Clone, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}
