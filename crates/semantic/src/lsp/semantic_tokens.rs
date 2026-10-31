//! Semantic-token decoding — the legend capture and the delta-encoded
//! `textDocument/semanticTokens/full` result format.

use serde_json::Value;

/// The server's semantic-token legend: token-type and modifier names by
/// index, captured from the initialize result's capabilities.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SemanticLegend {
    pub token_types: Vec<String>,
    pub token_modifiers: Vec<String>,
}

/// One decoded semantic token.  Positions stay UTF-16 as the server sent
/// them; the consumer converts to byte offsets against the buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticToken {
    pub line: usize,
    pub col_utf16: usize,
    pub len_utf16: usize,
    pub token_type: String,
    pub modifiers: Vec<String>,
}

fn legend_names(raw: Option<&Vec<Value>>) -> Vec<String> {
    raw.map(|names| {
        names
            .iter()
            .filter_map(|n| n.as_str().map(String::from))
            .collect()
    })
    .unwrap_or_default()
}

/// Pull the legend out of an initialize result.  A server without
/// semanticTokens capabilities decodes to an empty legend; every token it
/// later reports degrades to a type-index name.
pub fn semantic_token_legend(initialize_result: &Value) -> SemanticLegend {
    let legend = initialize_result.pointer("/capabilities/semanticTokensProvider/legend");
    match legend {
        Some(l) => SemanticLegend {
            token_types: legend_names(l.get("tokenTypes").and_then(Value::as_array)),
            token_modifiers: legend_names(l.get("tokenModifiers").and_then(Value::as_array)),
        },
        None => SemanticLegend::default(),
    }
}

/// Decode a `textDocument/semanticTokens/full` result: relative
/// quintuples (delta line, delta start, length, type index, modifier bits)
/// resolved through the server legend.  Unknown type indices keep their
/// number as the name so nothing silently drops.
pub fn decode_semantic_tokens(result: &Value, legend: &SemanticLegend) -> Vec<SemanticToken> {
    let Some(data) = result.get("data").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut tokens = Vec::with_capacity(data.len() / 5);
    let quintuples = data.chunks_exact(5);
    for chunk in quintuples {
        let nums: Option<Vec<u64>> = chunk.iter().map(Value::as_u64).collect();
        let Some(nums) = nums else { continue };
        if nums.len() != 5 {
            continue;
        }
        let d_line = nums[0];
        let d_start = nums[1];
        let len = nums[2];
        let ty = nums[3];
        let mods = nums[4];
        let token_type = legend
            .token_types
            .get(ty as usize)
            .cloned()
            .unwrap_or_else(|| ty.to_string());
        let modifiers: Vec<String> = legend
            .token_modifiers
            .iter()
            .enumerate()
            .filter(|(bit, _)| mods & (1 << bit) != 0)
            .map(|(_, name)| name.clone())
            .collect();
        tokens.push(SemanticToken {
            line: d_line as usize,
            col_utf16: d_start as usize,
            len_utf16: len as usize,
            token_type,
            modifiers,
        });
    }
    tokens
}

#[cfg(test)]
mod semantic_token_tests {
    use super::*;

    fn legend() -> SemanticLegend {
        SemanticLegend {
            token_types: vec![
                "namespace".into(),
                "type".into(),
                "variable".into(),
                "parameter".into(),
            ],
            token_modifiers: vec!["declaration".into(), "unreferenced".into()],
        }
    }

    #[test]
    fn legend_comes_from_initialize_capabilities() {
        let result = serde_json::json!({
            "capabilities": {
                "semanticTokensProvider": {
                    "legend": {
                        "tokenTypes": ["type", "variable"],
                        "tokenModifiers": ["deprecated"]
                    }
                }
            }
        });
        let l = semantic_token_legend(&result);
        assert_eq!(
            l.token_types,
            vec!["type".to_string(), "variable".to_string()]
        );
        assert_eq!(l.token_modifiers, vec!["deprecated".to_string()]);
    }

    #[test]
    fn missing_legend_decodes_empty() {
        assert_eq!(
            semantic_token_legend(&serde_json::json!({})),
            SemanticLegend::default()
        );
    }

    #[test]
    fn delta_quintuples_decode_to_absolute_positions() {
        // Two tokens on line 5: (line 0 → 5, col 0 → 8, len 3, type 2, mods bit1)
        let result = serde_json::json!({ "data": [5, 0, 3, 2, 2, 0, 12, 5, 1, 1] });
        let tokens = decode_semantic_tokens(&result, &legend());
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].line, 5);
        assert_eq!(tokens[0].col_utf16, 0);
        assert_eq!(tokens[0].len_utf16, 3);
        assert_eq!(tokens[0].token_type, "variable");
        assert_eq!(tokens[0].modifiers, vec!["unreferenced".to_string()]);
        assert_eq!(tokens[1].token_type, "type");
        assert_eq!(tokens[1].modifiers, vec!["declaration".to_string()]);
    }

    #[test]
    fn unknown_type_index_keeps_its_number() {
        let result = serde_json::json!({ "data": [0, 0, 1, 99, 0] });
        let tokens = decode_semantic_tokens(&result, &legend());
        assert_eq!(tokens[0].token_type, "99");
    }

    #[test]
    fn ragged_or_missing_data_yields_no_tokens() {
        assert!(decode_semantic_tokens(&serde_json::json!({}), &legend()).is_empty());
        assert!(
            decode_semantic_tokens(&serde_json::json!({ "data": [1, 2, 3] }), &legend()).is_empty()
        );
    }
}
