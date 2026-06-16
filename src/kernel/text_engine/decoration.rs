//! Decoration type — per-line buffer decorations added by extensions.

/// A per-line buffer decoration added by an extension.
///
/// Decorations are stored in named layers (`decoration_layers` on BufferView) so
/// multiple extensions can coexist without overwriting each other.
#[derive(Debug, Clone)]
pub enum Decoration {
    /// Text overlaid at a buffer position without displacing existing content.
    InlineText { line: usize, col: usize, text: String, face: String },
    /// Text appended after the last character on the line (padded by one space).
    EndOfLine { line: usize, text: String, face: String },
    /// Text rendered in a left-margin column reserved by `render_frame`.
    LinePrefix { line: usize, text: String, face: String },
}
