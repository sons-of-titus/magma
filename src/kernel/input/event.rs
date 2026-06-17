//! Input event types shared between all renderer backends.
//!
//! `InputEvent` is the canonical wire format between adapters (TUI/GUI) and
//! `input::dispatch`.

/// A normalised input event produced by any renderer backend.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// A key press or repeat, canonically serialised as a string
    /// (e.g. `"ctrl-a"`, `"esc"`, `"a"`, `"shift-tab"`).
    Key(String),
    /// Terminal / window resize.
    Resize(u16, u16),
    /// A mouse event at terminal column / row coordinates.
    Mouse(MouseEvent),
}

/// A mouse event produced by a renderer backend.
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
