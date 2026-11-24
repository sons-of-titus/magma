//! Picker sources — plugin-contributed search surfaces for the frontend's
//! picker overlay.
//!
//! The shell's picker has a fixed set of built-in kinds (files, symbols,
//! buffers, …).  A plugin registers a *source*: an id plus display
//! metadata.  Opening the source queues a `picker-source:<id>` UI request;
//! on every query change the frontend emits `picker-source-query`, a
//! Janet collector answers with `picker/push-items`, and the frontend
//! projects the cached entries next frame.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerSource {
    pub id: String,
    pub title: String,
    pub placeholder: String,
    /// Owning plugin, recorded at registration for teardown.
    pub owner: Option<String>,
}

#[derive(Debug, Default)]
pub struct PickerSourceRegistry {
    sources: Vec<PickerSource>,
}

impl PickerSourceRegistry {
    /// Insert or replace by id.
    pub fn register(&mut self, source: PickerSource) {
        self.sources.retain(|s| s.id != source.id);
        self.sources.push(source);
    }

    pub fn get(&self, id: &str) -> Option<&PickerSource> {
        self.sources.iter().find(|s| s.id == id)
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.sources.len();
        self.sources.retain(|s| s.id != id);
        before != self.sources.len()
    }

    /// Remove everything an owner registered; returns the removed ids.
    pub fn remove_owner(&mut self, owner: &str) -> Vec<String> {
        let mut removed = Vec::new();
        self.sources.retain(|s| {
            if s.owner.as_deref() == Some(owner) {
                removed.push(s.id.clone());
                false
            } else {
                true
            }
        });
        removed
    }

    pub fn list(&self) -> &[PickerSource] {
        &self.sources
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

/// One picker entry a collector pushed for the current query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerEntry {
    pub title: String,
    pub detail: String,
    pub meta: String,
    /// What selecting it does: `path<SHELF>line` opens the file at the
    /// line, `cmd:<command line>` runs the command line.
    pub payload: String,
}

/// Cache of the current source's answer for the current query; the
/// frontend projects it every frame while a source picker is open.
#[derive(Debug, Clone, Default)]
pub struct PickerState {
    pub items: Vec<PickerEntry>,
}

/// The payload separator shared with the frontend's file-kind payloads.
pub const PAYLOAD_SEP: char = '\u{1f}';

#[cfg(test)]
mod tests {
    use super::*;

    fn source(id: &str, owner: &str) -> PickerSource {
        PickerSource {
            id: id.to_string(),
            title: id.to_string(),
            placeholder: String::new(),
            owner: Some(owner.to_string()),
        }
    }

    #[test]
    fn register_replaces_by_id() {
        let mut reg = PickerSourceRegistry::default();
        reg.register(source("doc", "a"));
        reg.register(PickerSource {
            id: "doc".to_string(),
            title: "Document Symbols".to_string(),
            placeholder: String::new(),
            owner: Some("a".to_string()),
        });
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.get("doc").unwrap().title, "Document Symbols");
    }

    #[test]
    fn remove_owner_drops_only_that_owner() {
        let mut reg = PickerSourceRegistry::default();
        reg.register(source("doc", "a"));
        reg.register(source("tags", "a"));
        reg.register(source("other", "b"));
        let removed = reg.remove_owner("a");
        assert_eq!(removed, vec!["doc".to_string(), "tags".to_string()]);
        assert_eq!(reg.len(), 1);
        assert!(reg.get("other").is_some());
    }
}
