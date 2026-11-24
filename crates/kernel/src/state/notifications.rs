//! Notification center — a stack of dismissable messages.
//!
//! The single transient status message evaporates by design; failures a
//! user must not miss (missing language server, failing formatter, a
//! plugin that did not load) post here and stay until dismissed or
//! cleared.  The frontend projects the stack; headless embedders only
//! ever see the state.

use std::collections::VecDeque;

/// Newest-last; the frontend renders newest-first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub id: u64,
    /// "info" | "warn" | "error" — the card's tint.
    pub kind: String,
    pub title: String,
    pub body: String,
}

/// How many notifications the stack holds.  A flood of failures must not
/// become an unbounded log; oldest entries drop first.
const CAP: usize = 20;

#[derive(Debug, Default)]
pub struct NotificationCenter {
    next_id: u64,
    items: VecDeque<Notification>,
}

impl NotificationCenter {
    /// Post a notification; returns its id.
    pub fn post(&mut self, kind: &str, title: String, body: String) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.items.push_back(Notification {
            id,
            kind: kind.to_string(),
            title,
            body,
        });
        while self.items.len() > CAP {
            self.items.pop_front();
        }
        id
    }

    /// Dismiss by id; false when the id is gone (already dismissed, or
    /// dropped by the cap).
    pub fn dismiss(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|n| n.id != id);
        before != self.items.len()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn list(&self) -> &VecDeque<Notification> {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_assigns_growing_ids() {
        let mut c = NotificationCenter::default();
        let a = c.post("info", "one".into(), String::new());
        let b = c.post("error", "two".into(), String::new());
        assert!(b > a);
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn dismiss_removes_by_id_and_reports_unknowns() {
        let mut c = NotificationCenter::default();
        let id = c.post("warn", "w".into(), String::new());
        assert!(c.dismiss(id));
        assert_eq!(c.len(), 0);
        assert!(!c.dismiss(id), "already gone");
        assert!(!c.dismiss(999), "never existed");
    }

    #[test]
    fn the_cap_drops_the_oldest() {
        let mut c = NotificationCenter::default();
        let first = c.post("info", "first".into(), String::new());
        for i in 0..CAP {
            c.post("info", format!("n{i}"), String::new());
        }
        assert_eq!(c.len(), CAP);
        assert!(!c.dismiss(first), "the oldest fell off the cap");
        assert_eq!(c.list().front().unwrap().title, "n0");
    }

    #[test]
    fn clear_empties_everything() {
        let mut c = NotificationCenter::default();
        c.post("info", "x".into(), String::new());
        c.clear();
        assert!(c.is_empty());
    }
}
