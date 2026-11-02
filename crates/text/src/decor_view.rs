//! Decoration layers on a `BufferView` — line/col-keyed overlays (inlay
//! text, EOL notes, gutter prefixes) and their transaction transform.
//!
//! Decorations are stored by line number but tracked by byte offset
//! across edits: every mutation captures each decoration's byte position
//! before committing, then re-derives (line, col) through the same
//! anchor transform the cursor uses.  Without this, an insert above
//! leaves every decoration one line low until its owner re-publishes.

use super::anchor::{Affinity, Anchor};
use super::decoration::Decoration;
use super::transaction::EditTransaction;
use super::view::BufferView;

impl BufferView {
    // ── Mutation API ──

    pub fn decor_set_inline(
        &mut self,
        layer: &str,
        line: usize,
        col: usize,
        text: String,
        face: String,
    ) {
        let v = self.decoration_layers.entry(layer.to_string()).or_default();
        v.retain(|d| {
            !matches!(d, Decoration::InlineText { line: l, col: c, .. } if *l == line && *c == col)
        });
        v.push(Decoration::InlineText {
            line,
            col,
            text,
            face,
        });
    }

    pub fn decor_set_eol(&mut self, layer: &str, line: usize, text: String, face: String) {
        let v = self.decoration_layers.entry(layer.to_string()).or_default();
        v.retain(
            |d| !matches!(d, Decoration::EndOfLine { line: l, .. } if *l == line),
        );
        v.push(Decoration::EndOfLine { line, text, face });
    }

    pub fn decor_set_prefix(&mut self, layer: &str, line: usize, text: String, face: String) {
        let v = self.decoration_layers.entry(layer.to_string()).or_default();
        v.retain(
            |d| !matches!(d, Decoration::LinePrefix { line: l, .. } if *l == line),
        );
        v.push(Decoration::LinePrefix { line, text, face });
    }

    pub fn decor_clear_layer(&mut self, layer: &str) {
        self.decoration_layers.remove(layer);
    }

    pub fn decor_clear(&mut self) {
        self.decoration_layers.clear();
    }

    pub fn decor_count_layer(&self, layer: &str) -> usize {
        self.decoration_layers
            .get(layer)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    // ── Transaction transform ──

    /// Byte offset of every decoration in the *current* buffer: line
    /// start plus the byte length of `col` leading chars for inline
    /// decorations.  Call before committing a transaction.
    pub(crate) fn decor_byte_offsets(&self) -> Vec<(String, usize, usize)> {
        let buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::new();
        for (layer, decs) in &self.decoration_layers {
            for (i, d) in decs.iter().enumerate() {
                let (line, col) = match d {
                    Decoration::InlineText { line, col, .. } => (*line, Some(*col)),
                    Decoration::EndOfLine { line, .. } | Decoration::LinePrefix { line, .. } => {
                        (*line, None)
                    }
                };
                let Some(start) = buf.line_start_offset(line) else {
                    continue;
                };
                let byte = match col {
                    Some(c) if c > 0 => {
                        let end = buf
                            .line_start_offset(line + 1)
                            .unwrap_or_else(|| buf.len());
                        let line_text = buf.slice(start, end);
                        start + line_text
                            .chars()
                            .take(c)
                            .map(|ch| ch.len_utf8())
                            .sum::<usize>()
                    }
                    _ => start,
                };
                out.push((layer.clone(), i, byte));
            }
        }
        out
    }

    /// Re-derive every captured decoration's (line, col) through `txns`
    /// with the cursor's right-sticky anchor semantics: insertions at the
    /// anchored position push it down/along, deletions collapse it to the
    /// deletion start.  Call after the transactions committed.
    pub(crate) fn remap_decorations(
        &mut self,
        captured: &[(String, usize, usize)],
        txns: &[EditTransaction],
    ) {
        if txns.is_empty() {
            return;
        }
        let mut moved: Vec<(String, usize, usize)> = Vec::with_capacity(captured.len());
        for (layer, idx, byte) in captured {
            let mut pos = *byte;
            for txn in txns {
                pos = txn
                    .transform_anchor(Anchor {
                        offset: pos,
                        affinity: Affinity::After,
                    })
                    .offset;
            }
            moved.push((layer.clone(), *idx, pos));
        }
        let buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        for (layer, idx, pos) in moved {
            let Some(decs) = self.decoration_layers.get_mut(&layer) else {
                continue;
            };
            let Some(d) = decs.get_mut(idx) else {
                continue;
            };
            let p = buf.position_of(pos);
            match d {
                Decoration::InlineText { line, col, .. } => {
                    *line = p.line;
                    *col = p.column;
                }
                Decoration::EndOfLine { line, .. } | Decoration::LinePrefix { line, .. } => {
                    *line = p.line;
                }
            }
        }
    }
}
