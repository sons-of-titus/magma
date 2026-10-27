//! Text engine — the buffer layer the kernel composes.
//!
//! Every mutation is one canonical `Document::commit` (docs/core-redesign.md
//! §2.1); marks transform through transactions; undo history stores
//! transactions grouped explicitly; save state is revision-based.

pub mod diagnostic;
pub mod rope;
pub mod scan;
pub mod undo;
pub mod marks;
pub mod decoration;
pub mod position;
pub mod view;
pub mod buffer;
mod breakpoint_anchors;
pub use buffer::Buffer;
pub use buffer::ImageData;
mod fold;

// Phase-1 primitives (docs/core-redesign.md): the transaction core below
// `Buffer`, exercised by both this crate's tests and the editor integration
// tests that drive full buffer workloads.
#[allow(dead_code)]
pub mod anchor;
#[allow(dead_code)]
pub mod document;
#[allow(dead_code)]
pub mod edit;
#[allow(dead_code)]
pub mod line_index;
#[allow(dead_code)]
pub mod revision;
#[allow(dead_code)]
pub mod transaction;
pub mod text_layout;

pub use decoration::Decoration;
pub use position::Position;
pub use view::BufferView;


/// Buffer major mode — the content/context type of a buffer (Emacs-style).
///
/// The Rust core defines only structural parent modes.  Language-specific
/// modes (rust-mode, python-mode, …) are defined by Janet extensions and
/// stored as `Custom(String)`.  One major mode is active per buffer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MajorMode {
    /// No special mode; plain editing with no added keybindings.
    #[default]
    Fundamental,
    /// Parent mode for all human-readable prose buffers.
    Text,
    /// Parent mode for all programming-language buffers.
    Prog,
    /// Extension-defined mode (e.g. "rust-mode", "markdown-mode").
    Custom(String),
}

impl MajorMode {
    pub fn name(&self) -> &str {
        match self {
            MajorMode::Fundamental => "fundamental",
            MajorMode::Text        => "text",
            MajorMode::Prog        => "prog",
            MajorMode::Custom(s)   => s.as_str(),
        }
    }

    /// Parse a major mode from a string supplied by the user or an extension.
    /// Anything the core doesn't recognise becomes `Custom`.
    pub fn from_name(s: &str) -> Self {
        match s {
            "fundamental" | "fundamental-mode" => MajorMode::Fundamental,
            "text"        | "text-mode"        => MajorMode::Text,
            "prog"        | "prog-mode"        => MajorMode::Prog,
            other                              => MajorMode::Custom(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests;
