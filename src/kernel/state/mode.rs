//! Generic editor mode model — `EditorMode`, `Selection`.
//!
//! Magma ships non-modal: the default mode accepts text and editing happens
//! through chord-bound commands.  The struct is extension substrate — a
//! community modal-editing plugin may rename the mode and manage keymap
//! layers, but nothing in the core enters a mode.

/// Search direction — used by the search commands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

/// The current editor context.
///
/// `name` is opaque to the core: the shipped value is `"editor"`, and a
/// focused terminal buffer sets `"terminal"`.  A community plugin may
/// introduce other names, but no core code path branches on them.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorMode {
    /// Opaque mode name chosen by the active editing plugin.
    pub name: String,
    /// When `true`, unbound keys are inserted as text instead of being dropped.
    pub accepts_text: bool,
}

impl EditorMode {
    pub fn new(name: impl Into<String>, accepts_text: bool) -> Self {
        Self { name: name.into(), accepts_text }
    }

    /// True when the mode name equals `name` (case-sensitive).
    pub fn is_named(&self, name: &str) -> bool { self.name == name }
}

impl Default for EditorMode {
    /// The one shipped editing mode: text input always on.
    fn default() -> Self { Self::new("editor", true) }
}

/// A generic visual selection.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    /// Byte offset where the selection began.
    pub anchor: usize,
    /// Kind string: `"char"`, `"line"`, `"block"`, or any plugin-defined value.
    pub kind: String,
}

impl Selection {
    pub fn char(anchor: usize)  -> Self { Self { anchor, kind: "char".into()  } }
    pub fn line(anchor: usize)  -> Self { Self { anchor, kind: "line".into()  } }
    pub fn block(anchor: usize) -> Self { Self { anchor, kind: "block".into() } }
    pub fn is_line(&self)  -> bool { self.kind == "line"  }
    pub fn is_block(&self) -> bool { self.kind == "block" }
}
