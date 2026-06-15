//! Janet C function for applying LSP WorkspaceEdit JSON.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::lsp::parse;

/// (lsp/apply-edit edit-json) → nil
unsafe extern "C-unwind" fn c_lsp_apply_edit(argc: i32, argv: *mut Janet) -> Janet { unsafe {
    let edit_json = match conv::get_str(argc, argv, 0) {
        Some(j) => j,
        None => return conv::nil(),
    };

    with_editor(|ed| {
        let json: serde_json::Value = match serde_json::from_str(&edit_json) {
            Ok(v) => v,
            Err(e) => {
                debug!("lsp/apply-edit: invalid JSON: {e}");
                return conv::nil();
            }
        };

        let mut edits_by_uri: std::collections::HashMap<String, Vec<(usize, usize, String)>> =
            std::collections::HashMap::new();

        if let Some(changes) = json.get("changes").and_then(|c| c.as_object()) {
            for (uri, text_edits) in changes {
                let path = parse::uri_to_path(uri);
                if let Some(edits) = text_edits.as_array() {
                    for edit in edits {
                        if let Some((start, end, new_text)) = parse_text_edit(edit) {
                            edits_by_uri.entry(path.clone()).or_default().push((start, end, new_text));
                        }
                    }
                }
            }
        }

        if let Some(doc_changes) = json.get("documentChanges").and_then(|d| d.as_array()) {
            for entry in doc_changes {
                if let Some(text_doc) = entry.get("textDocument")
                    && let Some(raw_uri) = text_doc.get("uri").and_then(|u| u.as_str()) {
                        let path = parse::uri_to_path(raw_uri);
                        if let Some(edits) = entry.get("edits").and_then(|e| e.as_array()) {
                            for e in edits {
                                if let Some((start, end, new_text)) = parse_text_edit(e) {
                                    edits_by_uri.entry(path.clone()).or_default().push((start, end, new_text));
                                }
                            }
                        }
                    }
            }
        }

        for (path, mut edits) in edits_by_uri {
            edits.sort_by_key(|b| std::cmp::Reverse(b.0));
            for (_, buf) in ed.buffers.iter_mut() {
                if buf.path.as_deref() == Some(&path) {
                    for (start, end, new_text) in &edits {
                        let start = *start;
                        let end = *end;
                        if end > buf.len() || start > end { continue; }
                        if start < end { buf.delete(start, end); }
                        if !new_text.is_empty() { buf.insert(start, new_text); }
                    }
                    break;
                }
            }
        }

        conv::nil()
    })
}}

fn parse_text_edit(edit: &serde_json::Value) -> Option<(usize, usize, String)> {
    let range = edit.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;
    let start_line = start.get("line")?.as_u64()? as usize;
    let start_char = start.get("character")?.as_u64()? as usize;
    let end_line = end.get("line")?.as_u64()? as usize;
    let end_char = end.get("character")?.as_u64()? as usize;
    let new_text = edit.get("newText").and_then(|t| t.as_str()).unwrap_or("");
    let start_offset = start_line * 80 + start_char;
    let end_offset = end_line * 80 + end_char;
    Some((start_offset, end_offset, new_text.to_string()))
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"lsp/apply-edit".as_ptr() as *const _,
            cfun: Some(c_lsp_apply_edit as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Apply a WorkspaceEdit JSON object to open buffers".as_ptr() as *const _,
        },
    ]
}
