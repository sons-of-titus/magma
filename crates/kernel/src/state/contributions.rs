//! Per-plugin record of script-level registrations.
//!
//! Registry-level contributions (languages, templates, toolchains) carry
//! their owner and unregister by it.  What a plugin's entry *script*
//! registers — commands, event subscriptions, keymap bindings — has no
//! owner on its own, so the registration-time context records it here and
//! `plugin/unload` takes the journal to remove them.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Contribution {
    Command(String),
    Event { name: String, subscription: u64 },
    Keymap { layer: Option<String>, key: String },
    PickerSource(String),
}

#[derive(Default)]
pub struct ContributionJournal {
    by_plugin: HashMap<String, Vec<Contribution>>,
}

impl ContributionJournal {
    pub fn record(&mut self, plugin: &str, contribution: Contribution) {
        self.by_plugin
            .entry(plugin.to_string())
            .or_default()
            .push(contribution);
    }

    /// Remove and return everything `plugin` recorded; the core never
    /// re-records for a plugin whose unload is in progress.
    pub fn take(&mut self, plugin: &str) -> Vec<Contribution> {
        self.by_plugin.remove(plugin).unwrap_or_default()
    }

    pub fn count(&self, plugin: &str) -> usize {
        self.by_plugin.get(plugin).map_or(0, |v| v.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_removes_only_the_named_plugin() {
        let mut journal = ContributionJournal::default();
        journal.record("a", Contribution::Command("a-cmd".into()));
        journal.record("a", Contribution::Command("a-cmd-2".into()));
        journal.record("b", Contribution::Command("b-cmd".into()));

        assert_eq!(journal.count("a"), 2);
        let taken = journal.take("a");
        assert_eq!(taken.len(), 2);
        assert_eq!(journal.count("a"), 0, "take drains the plugin's journal");
        assert_eq!(journal.count("b"), 1, "other plugins are untouched");
        assert_eq!(journal.take("a"), Vec::new(), "take is idempotent");
    }

    #[test]
    fn core_registrations_are_never_recorded() {
        // The journal only ever sees entries pushed under an Extension
        // context; there is no "core" key by construction (callers check
        // the context before recording).  This pins the empty default.
        let journal = ContributionJournal::default();
        assert_eq!(journal.count("anything"), 0);
    }
}
