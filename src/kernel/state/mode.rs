//! Generic editor mode model — `EditorMode`, `Minibuffer`, `Selection`.
//!
//! Rust never branches on mode names; all mode vocabulary is owned by Janet.

/// Search direction — used by vim search builtin commands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

/// The complete, editing-paradigm-agnostic description of the current editor mode.
///
/// Rust never branches on `name`.  The vim plugin sets `name = "normal"`,
/// `"insert"`, `"visual"`, etc., but these strings have no special meaning in
/// the Rust core — they are opaque labels owned entirely by Janet.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorMode {
    /// Opaque mode name chosen by the active editing plugin.
    pub name: String,
    /// When `true`, unbound keys are inserted as text instead of being dropped.
    pub accepts_text: bool,
    /// Active minibuffer, if any.  When `Some`, unbound keys feed `input`
    /// rather than the buffer; Rust draws prompt+input at the bottom of the screen.
    pub minibuffer: Option<Minibuffer>,
}

impl EditorMode {
    pub fn new(name: impl Into<String>, accepts_text: bool) -> Self {
        Self { name: name.into(), accepts_text, minibuffer: None }
    }

    /// True when the mode name equals `name` (case-sensitive).
    pub fn is_named(&self, name: &str) -> bool { self.name == name }
}

impl Default for EditorMode {
    fn default() -> Self { Self::new("normal", false) }
}

/// A prompt+input widget displayed at the bottom of the screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Minibuffer {
    pub prompt: String,
    pub input:  String,
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
