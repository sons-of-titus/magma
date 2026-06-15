//! Decoration types and Buffer decoration methods.

use super::Buffer;

/// A per-line buffer decoration added by an extension.
///
/// Decorations are stored in named layers (`decoration_layers`) so multiple
/// extensions can coexist without overwriting each other.
#[derive(Debug, Clone)]
pub enum Decoration {
    /// Text overlaid at a buffer position without displacing existing content.
    InlineText { line: usize, col: usize, text: String, face: String },
    /// Text appended after the last character on the line (padded by one space).
    EndOfLine { line: usize, text: String, face: String },
    /// Text rendered in a left-margin column reserved by `render_frame`.
    LinePrefix { line: usize, text: String, face: String },
}

impl Buffer {
    pub fn decor_set_inline(&mut self, layer: &str, line: usize, col: usize, text: String, face: String) {
        let layer_vec = self.decoration_layers.entry(layer.to_string()).or_default();
        layer_vec.retain(|d| !matches!(d, Decoration::InlineText { line: l, col: c, .. } if *l == line && *c == col));
        layer_vec.push(Decoration::InlineText { line, col, text, face });
    }

    pub fn decor_set_eol(&mut self, layer: &str, line: usize, text: String, face: String) {
        let layer_vec = self.decoration_layers.entry(layer.to_string()).or_default();
        layer_vec.retain(|d| !matches!(d, Decoration::EndOfLine { line: l, .. } if *l == line));
        layer_vec.push(Decoration::EndOfLine { line, text, face });
    }

    pub fn decor_set_prefix(&mut self, layer: &str, line: usize, text: String, face: String) {
        let layer_vec = self.decoration_layers.entry(layer.to_string()).or_default();
        layer_vec.retain(|d| !matches!(d, Decoration::LinePrefix { line: l, .. } if *l == line));
        layer_vec.push(Decoration::LinePrefix { line, text, face });
    }

    pub fn decor_clear_layer(&mut self, layer: &str) {
        self.decoration_layers.remove(layer);
    }

    pub fn decor_clear(&mut self) {
        self.decoration_layers.clear();
    }

    pub fn decor_count_layer(&self, layer: &str) -> usize {
        self.decoration_layers.get(layer).map(|v| v.len()).unwrap_or(0)
    }
}
