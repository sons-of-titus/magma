//! Offer queue — questions the editor asks before doing something the
//! user should decide (install a pinned server, enable a project's
//! plugin).  Janet raises an offer, the frontend shows it as a modal,
//! and the answer comes back as an `offer-resolved` command; nothing
//! downloads or loads until the user accepts.

/// One pending question.  `kind` selects the accept-side behavior
/// ("server", "toolchain", "plugin"); `id` names the language, slot, or
/// plugin the decision applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Default)]
pub struct OfferQueue {
    items: Vec<Offer>,
}

impl OfferQueue {
    /// Queue an offer; a repeat of the same `(kind, id)` while one is
    /// still pending is a no-op so per-buffer triggers cannot stack the
    /// same question.
    pub fn post(&mut self, offer: Offer) {
        if self.items.iter().any(|o| o.kind == offer.kind && o.id == offer.id) {
            return;
        }
        self.items.push(offer);
    }

    /// Is an offer with this `(kind, id)` waiting?
    pub fn is_pending(&self, kind: &str, id: &str) -> bool {
        self.items.iter().any(|o| o.kind == kind && o.id == id)
    }

    /// Pop the front offer (the one the modal is showing).
    pub fn take(&mut self) -> Option<Offer> {
        if self.items.is_empty() {
            None
        } else {
            Some(self.items.remove(0))
        }
    }

    pub fn front(&self) -> Option<&Offer> {
        self.items.first()
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

    fn offer(kind: &str, id: &str) -> Offer {
        Offer {
            kind: kind.into(),
            id: id.into(),
            title: "Install?".into(),
            body: String::new(),
        }
    }

    #[test]
    fn posting_twice_while_pending_dedupes() {
        let mut q = OfferQueue::default();
        q.post(offer("server", "rust"));
        q.post(offer("server", "rust"));
        assert_eq!(q.len(), 1, "a pending offer must not stack");
    }

    #[test]
    fn take_pops_in_order_and_reports_pending() {
        let mut q = OfferQueue::default();
        q.post(offer("server", "a"));
        q.post(offer("plugin", "b"));
        assert!(q.is_pending("server", "a"));
        assert!(!q.is_pending("server", "zz"));
        let first = q.take().unwrap();
        assert_eq!((first.kind.as_str(), first.id.as_str()), ("server", "a"));
        assert!(!q.is_pending("server", "a"), "taken offers stop being pending");
        assert!(q.take().is_some());
        assert!(q.take().is_none());
    }
}
