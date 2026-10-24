//! Apply LSP WorkspaceEdits to open buffers — the shared core of the
//! Janet `(lsp/apply-edit)` fn, inline code actions, and server-driven
//! `workspace/applyEdit` requests.

use crate::kernel::state::Editor;
use crate::kernel::text_engine::Buffer;

/// An LSP TextEdit range in LSP coordinates: `(line, utf16 character)`.
struct EditRange {
    start: (usize, usize),
    end: (usize, usize),
    new_text: String,
}

/// Apply a parsed WorkspaceEdit to open buffers.
pub fn apply_workspace_edit(ed: &mut Editor, json: &serde_json::Value) {
    let mut edits_by_uri: std::collections::HashMap<String, Vec<EditRange>> =
        std::collections::HashMap::new();

    if let Some(changes) = json.get("changes").and_then(|c| c.as_object()) {
        for (uri, text_edits) in changes {
            let path = super::lsp_proto::uri_to_path(uri);
            if let Some(edits) = text_edits.as_array() {
                for edit in edits {
                    if let Some(range) = parse_text_edit(edit) {
                        edits_by_uri.entry(path.clone()).or_default().push(range);
                    }
                }
            }
        }
    }

    if let Some(doc_changes) = json.get("documentChanges").and_then(|d| d.as_array()) {
        for entry in doc_changes {
            if let Some(text_doc) = entry.get("textDocument")
                && let Some(raw_uri) = text_doc.get("uri").and_then(|u| u.as_str()) {
                    let path = super::lsp_proto::uri_to_path(raw_uri);
                    if let Some(edits) = entry.get("edits").and_then(|e| e.as_array()) {
                        for e in edits {
                            if let Some(range) = parse_text_edit(e) {
                                edits_by_uri.entry(path.clone()).or_default().push(range);
                            }
                        }
                    }
                }
        }
    }

    for (path, ranges) in edits_by_uri {
        let key = ed.buffers.iter()
            .find(|(_, arc)| arc.lock().unwrap().path.as_deref() == Some(&path))
            .map(|(k, _)| k);
        let Some(key) = key else { continue };
        let Some(view) = ed.view_of_buffer_mut(key) else { continue };

        // Resolve LSP (line, character) positions against THIS buffer's
        // line index before touching anything.
        let buffer = view.buffer.clone();
        let buf = buffer.lock().unwrap();
        let mut spans: Vec<(usize, usize, String)> = ranges.into_iter()
            .map(|r| (
                offset_of_lsp_position(&buf, r.start.0, r.start.1),
                offset_of_lsp_position(&buf, r.end.0, r.end.1),
                r.new_text,
            ))
            .collect();
        drop(buf);

        // Back to front, so earlier ranges stay valid while applying.
        spans.sort_by_key(|b| std::cmp::Reverse(b.0));
        for (start, end, new_text) in spans {
            let buf_len = buffer.lock().unwrap().len();
            if end > buf_len || start > end { continue; }
            if start < end { view.delete(start, end); }
            if !new_text.is_empty() { view.insert(start, &new_text); }
        }
    }
}

fn parse_text_edit(edit: &serde_json::Value) -> Option<EditRange> {
    let range = edit.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;
    Some(EditRange {
        start: line_char(start)?,
        end: line_char(end)?,
        new_text: edit.get("newText").and_then(|t| t.as_str()).unwrap_or("").to_string(),
    })
}

fn line_char(pos: &serde_json::Value) -> Option<(usize, usize)> {
    Some((
        pos.get("line")?.as_u64()? as usize,
        pos.get("character")?.as_u64()? as usize,
    ))
}

/// Byte offset of an LSP (line, utf16-character) position in `buf`.
/// Positions past the end of a line or the document clamp.
pub fn offset_of_lsp_position(buf: &Buffer, line: usize, character: usize) -> usize {
    let len = buf.len();
    let Some(line_start) = buf.line_start_offset(line) else {
        return len; // line beyond the document
    };
    let line_end = buf.line_start_offset(line + 1).unwrap_or(len);
    let text = buf.slice(line_start, line_end);
    let mut utf16 = 0usize;
    for (byte_off, c) in text.char_indices() {
        if utf16 >= character {
            return line_start + byte_off;
        }
        utf16 += c.len_utf16();
    }
    line_end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer_of(text: &str) -> Buffer {
        crate::kernel::text_engine::Buffer::from_string(
            crate::kernel::state::id::BufferId(1), "t", text,
        )
    }

    #[test]
    fn offset_of_resolves_lines_and_utf16_columns() {
        let buf = buffer_of("hello\nwörld\nx");
        assert_eq!(offset_of_lsp_position(&buf, 0, 0), 0);
        assert_eq!(offset_of_lsp_position(&buf, 1, 0), 6);
        // LSP character 2 on "wörld" is the start of 'r': ö is 2 UTF-8
        // bytes but 1 UTF-16 unit.
        assert_eq!(offset_of_lsp_position(&buf, 1, 2), 9);
        assert_eq!(offset_of_lsp_position(&buf, 2, 0), 13);
    }

    #[test]
    fn offset_of_clamps_past_line_and_document() {
        let buf = buffer_of("ab\ncd");
        assert_eq!(offset_of_lsp_position(&buf, 0, 99), 3, "past end of line clamps to line end");
        assert_eq!(offset_of_lsp_position(&buf, 99, 0), buf.len(), "past document clamps to end");
    }
}
