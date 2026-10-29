//! Signature help — the call-signature popup while typing arguments.
//!
//! The input layer requests on `(` and `,` and drops on `)`; the response
//! lands through the LSP handler (`Event::SignatureHelp`), which parses and
//! stores the state here.  Like completions, per-keystroke requests
//! supersede their predecessor and stale revisions are dropped.

use serde_json::Value;

use crate::state::Editor;
use crate::state::signature::{SignatureInfo, SignatureState};

/// Ask the attached language server for signature help at the cursor.
pub fn request(editor: &Editor, buf_id: usize) -> bool {
    let Some(lang) = editor.lsp.language_for_buffer(buf_id) else {
        return false;
    };
    let Some(server) = editor.lsp.get(&lang) else {
        return false;
    };
    let Some(arc) = editor.buffers.get(buf_id) else {
        return false;
    };
    let (uri, position, revision) = {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let uri = match buf.path.as_deref() {
            Some(p) => format!("file://{p}"),
            None => return false,
        };
        let cursor = editor
            .view_of_buffer(buf_id)
            .map(|v| v.cursor.offset)
            .unwrap_or(0);
        (uri, buf.lsp_position(cursor.min(buf.len())), buf.revision())
    };
    let params = serde_json::json!({
        "textDocument": { "uri": uri },
        "position": { "line": position.0, "character": position.1 },
    });
    server.request("textDocument/signatureHelp", params, buf_id, revision);
    true
}

/// Follow call typing: request on `(`/`,`, drop on `)`.  With no server
/// attached there is nothing to show, so the popup clears.
pub fn auto_trigger(editor: &mut Editor, key: &str) {
    match key {
        "(" | "," => {
            let buf_id = editor
                .view_tree
                .focused_window()
                .and_then(|wid| editor.view_tree.buffer(wid));
            let requested = buf_id.map(|id| request(editor, id)).unwrap_or(false);
            if !requested {
                dismiss(editor);
            }
        }
        ")" => dismiss(editor),
        _ => {}
    }
}

pub fn dismiss(editor: &mut Editor) {
    editor.signature = SignatureState::default();
}

/// Parse a `textDocument/signatureHelp` result.  `null` (no signature at
/// this position) and empty results yield `None` so the popup hides.
pub fn parse_signature_help(json: &str) -> Option<SignatureState> {
    let v: Value = serde_json::from_str(json).ok()?;
    let arr = v.get("signatures")?.as_array()?;
    let mut signatures = Vec::new();
    for sig in arr {
        let label = sig.get("label")?.as_str()?.to_string();
        // Parameters live in the argument list: string-form labels are
        // searched after the opening paren so "max(a, b)" does not match
        // the 'a' inside the callee name.
        let after_paren = label.find('(').map(|i| i + 1).unwrap_or(0);
        let mut params = Vec::new();
        if let Some(list) = sig.get("parameters").and_then(Value::as_array) {
            for p in list {
                // A parameter label is either a [start, end] pair in UTF-16
                // code units of the signature label, or the substring itself.
                // A parameter that does not resolve is skipped, not fatal.
                let range = match p.get("label") {
                    Some(Value::Array(a)) if a.len() == 2 => {
                        let (Some(s), Some(e)) = (a[0].as_u64(), a[1].as_u64()) else {
                            continue;
                        };
                        utf16_to_byte(&label, s as usize)
                            .zip(utf16_to_byte(&label, e as usize))
                    }
                    Some(Value::String(s)) => {
                        let from = params.last().map(|&(_, end)| end).unwrap_or(after_paren);
                        label[from..]
                            .find(s.as_str())
                            .map(|i| (from + i, from + i + s.len()))
                            .or_else(|| label.find(s.as_str()).map(|i| (i, i + s.len())))
                    }
                    _ => None,
                };
                if let Some(r) = range {
                    params.push(r);
                }
            }
        }
        let docs = sig
            .get("documentation")
            .map(flatten_documentation)
            .unwrap_or_default();
        signatures.push(SignatureInfo {
            label,
            params,
            docs,
        });
    }
    if signatures.is_empty() {
        return None;
    }
    Some(SignatureState {
        active_signature: v
            .get("activeSignature")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
        active_parameter: v
            .get("activeParameter")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
        signatures,
        visible: true,
    })
}

/// LSP offsets are UTF-16 code units; the buffer-facing offsets are bytes.
fn utf16_to_byte(label: &str, target: usize) -> Option<usize> {
    let mut units = 0usize;
    for (i, c) in label.char_indices() {
        if units >= target {
            return Some(i);
        }
        units += c.len_utf16();
    }
    (units == target).then_some(label.len())
}

fn flatten_documentation(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(o) => o
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_extracts_signatures_params_and_active_indexes() {
        let json = r#"{
            "activeSignature": 1,
            "activeParameter": 0,
            "signatures": [
                { "label": "fn new() -> Foo" },
                { "label": "fn push(&mut self, value: T, n: usize)",
                  "parameters": [
                    { "label": [19, 27] },
                    { "label": [29, 37] }
                  ],
                  "documentation": { "kind": "markdown", "value": "Appends." } }
            ]
        }"#;
        let state = parse_signature_help(json).expect("a populated response parses");
        assert_eq!(state.signatures.len(), 2);
        assert_eq!(state.active_signature, 1);
        assert_eq!(state.active_parameter, 0);
        let sig = state.active().unwrap();
        assert_eq!(sig.params[0], (19, 27));
        assert_eq!(&sig.label[19..27], "value: T");
        assert_eq!(sig.params[1], (29, 37));
        assert_eq!(sig.docs, "Appends.");
        assert!(state.visible);
    }

    #[test]
    fn parse_handles_string_parameter_labels() {
        let json = r#"{ "signatures": [{ "label": "max(a, b)", "parameters": [{"label": "a"}, {"label": "b"}] }] }"#;
        let state = parse_signature_help(json).unwrap();
        assert_eq!(state.signatures[0].params, vec![(4, 5), (7, 8)]);
    }

    #[test]
    fn utf16_offsets_convert_through_surrogate_pairs() {
        // 🙂 is 4 bytes in UTF-8 but 2 UTF-16 code units, so byte offsets
        // drift past it: "arg" spans UTF-16 [7, 10] which is bytes [9, 12].
        let json = r#"{ "signatures": [{ "label": "fn(🙂, arg)", "parameters": [{ "label": [7, 10] }] }] }"#;
        let state = parse_signature_help(json).unwrap();
        let (start, end) = state.signatures[0].params[0];
        assert_eq!(&state.signatures[0].label[start..end], "arg");
    }

    #[test]
    fn an_unresolvable_parameter_is_skipped_not_fatal() {
        // The end offset is past the label (11 UTF-16 units total): the
        // parameter drops but the response still parses.
        let json = r#"{ "signatures": [{ "label": "fn(éü, arg)", "parameters": [{ "label": [9, 12] }] }] }"#;
        let state = parse_signature_help(json).unwrap();
        assert!(state.signatures[0].params.is_empty());
    }

    #[test]
    fn null_and_empty_responses_parse_to_none() {
        assert!(parse_signature_help("null").is_none());
        assert!(parse_signature_help(r#"{"signatures": []}"#).is_none());
        assert!(parse_signature_help("not json").is_none());
    }
}
