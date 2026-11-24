/// Snippet expansion state and the registered-snippet table.
#[derive(Debug, Clone, Default)]
pub struct SnippetState {
    pub active: bool,
    pub tabstops: Vec<crate::snippet::TabStop>,
    pub tabstop_idx: usize,
    /// Snippets contributed by plugins and users, language-scoped.
    pub registry: SnippetRegistry,
}

/// One registered snippet.  Tab-stop syntax is the LSP form the engine
/// already parses (`$N`, `${N:placeholder}`, `$0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub trigger: String,
    pub body: String,
    /// Language id the snippet serves; None serves every buffer.
    pub language: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SnippetRegistry {
    entries: Vec<Snippet>,
}

impl SnippetRegistry {
    /// Insert or replace.  A trigger is unique per language scope: the
    /// latest registration for `(trigger, language)` wins.
    pub fn register(&mut self, snippet: Snippet) {
        self.entries
            .retain(|s| !(s.trigger == snippet.trigger && s.language == snippet.language));
        self.entries.push(snippet);
    }

    pub fn remove(&mut self, trigger: &str, language: Option<&str>) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|s| !(s.trigger == trigger && s.language.as_deref() == language));
        before != self.entries.len()
    }

    /// Drop every snippet of a language — plugin teardown.  Global
    /// snippets (no language) survive: they are not the plugin's.
    pub fn remove_language(&mut self, language: &str) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|s| s.language.as_deref() != Some(language));
        before - self.entries.len()
    }

    /// The snippet for `trigger` in `language`: a language-scoped entry
    /// wins over a global one with the same trigger.
    pub fn get(&self, trigger: &str, language: Option<&str>) -> Option<&Snippet> {
        self.entries
            .iter()
            .find(|s| s.language.as_deref() == language && s.trigger == trigger)
            .or_else(|| {
                self.entries
                    .iter()
                    .find(|s| s.language.is_none() && s.trigger == trigger)
            })
    }

    /// All snippets serving `language`, ordered by trigger.
    pub fn for_language(&self, language: Option<&str>) -> Vec<&Snippet> {
        let mut v: Vec<&Snippet> = self
            .entries
            .iter()
            .filter(|s| s.language.is_none() || s.language.as_deref() == language)
            .collect();
        v.sort_by(|a, b| a.trigger.cmp(&b.trigger));
        v
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snip(trigger: &str, language: Option<&str>) -> Snippet {
        Snippet {
            trigger: trigger.to_string(),
            body: "body".to_string(),
            language: language.map(|s| s.to_string()),
            description: None,
        }
    }

    #[test]
    fn register_replaces_same_trigger_and_scope() {
        let mut reg = SnippetRegistry::default();
        reg.register(snip("t", Some("rust")));
        reg.register(Snippet {
            trigger: "t".to_string(),
            body: "new".to_string(),
            language: Some("rust".to_string()),
            description: None,
        });
        assert_eq!(reg.len(), 1, "re-registration replaces");
        assert_eq!(reg.get("t", Some("rust")).unwrap().body, "new");
    }

    #[test]
    fn language_scoped_wins_over_global() {
        let mut reg = SnippetRegistry::default();
        reg.register(Snippet {
            trigger: "t".to_string(),
            body: "global".to_string(),
            language: None,
            description: None,
        });
        reg.register(Snippet {
            trigger: "t".to_string(),
            body: "scoped".to_string(),
            language: Some("python".to_string()),
            description: None,
        });
        assert_eq!(reg.get("t", Some("python")).unwrap().body, "scoped");
        assert_eq!(reg.get("t", Some("rust")).unwrap().body, "global");
    }

    #[test]
    fn remove_language_drops_only_that_language() {
        let mut reg = SnippetRegistry::default();
        reg.register(snip("a", Some("rust")));
        reg.register(snip("b", Some("python")));
        reg.register(snip("c", None));
        assert_eq!(reg.remove_language("rust"), 1);
        assert!(reg.get("a", Some("rust")).is_none());
        assert!(reg.get("b", Some("python")).is_some());
        assert!(reg.get("c", None).is_some(), "global snippets survive");
    }

    #[test]
    fn for_language_includes_globals_sorted() {
        let mut reg = SnippetRegistry::default();
        reg.register(snip("zz", Some("rust")));
        reg.register(snip("aa", Some("rust")));
        reg.register(snip("mm", None));
        let triggers: Vec<&str> = reg
            .for_language(Some("rust"))
            .iter()
            .map(|s| s.trigger.as_str())
            .collect();
        assert_eq!(triggers, vec!["aa", "mm", "zz"]);
    }
}
