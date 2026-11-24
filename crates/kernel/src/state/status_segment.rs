//! Plugin-contributed status-bar segments.
//!
//! The bar's fixed pills (file, branch, position …) are chrome in the
//! frontend; anything a plugin wants visible — LSP server state, test
//! status, a per-language indicator — registers here and the shell
//! renders the segments between the fixed ones.

/// A status pill.  `kind` tints it ("info" | "warn" | "error"); an empty
/// text removes the segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSegment {
    pub id: String,
    pub text: String,
    pub kind: String,
}

/// Set or remove (`text` empty) a segment, keeping registration order.
pub fn set(segments: &mut Vec<StatusSegment>, segment: StatusSegment) {
    if segment.text.is_empty() {
        segments.retain(|s| s.id != segment.id);
        return;
    }
    match segments.iter_mut().find(|s| s.id == segment.id) {
        Some(slot) => *slot = segment,
        None => segments.push(segment),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(id: &str, text: &str) -> StatusSegment {
        StatusSegment {
            id: id.to_string(),
            text: text.to_string(),
            kind: "info".to_string(),
        }
    }

    #[test]
    fn set_upserts_in_registration_order() {
        let mut v = Vec::new();
        set(&mut v, seg("a", "one"));
        set(&mut v, seg("b", "two"));
        set(&mut v, seg("a", "uno"));
        let ids: Vec<&str> = v.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"], "insertion order survives updates");
        assert_eq!(v[0].text, "uno");
    }

    #[test]
    fn empty_text_removes() {
        let mut v = Vec::new();
        set(&mut v, seg("a", "one"));
        set(&mut v, seg("a", ""));
        assert!(v.is_empty(), "empty text is a removal, not a blank pill");
        set(&mut v, seg("ghost", ""));
        assert!(v.is_empty());
    }
}
