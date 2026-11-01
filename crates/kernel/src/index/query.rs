//! Symbol queries over the index store: prefix and ranked-substring lookup
//! for completion and the Go to Symbol picker.

use super::IndexStore;
use super::extract::IndexedSymbol;

impl IndexStore {
    /// All symbols whose name starts with `prefix` — workspace symbol
    /// search / completion augmentation.
    pub fn symbols_with_prefix(&self, prefix: &str) -> Vec<(String, &IndexedSymbol)> {
        let mut out = Vec::new();
        if prefix.is_empty() {
            return out;
        }
        for rec in self.records.values() {
            for sym in &rec.symbols {
                if sym.name.starts_with(prefix) {
                    out.push((rec.path.clone(), sym));
                    if out.len() >= 200 {
                        return out;
                    }
                }
            }
        }
        out.sort_by(|a, b| a.1.name.cmp(&b.1.name));
        out
    }

    /// Symbols whose name contains `needle` case-insensitively, best
    /// (shortest) names first — the Go to Symbol picker's query.
    pub fn symbols_matching(&self, needle: &str, limit: usize) -> Vec<(String, &IndexedSymbol)> {
        let mut out: Vec<(String, &IndexedSymbol)> = Vec::new();
        let needle_lower = needle.to_lowercase();
        if needle_lower.is_empty() {
            return out;
        }
        for rec in self.records.values() {
            for sym in &rec.symbols {
                if sym.name.to_lowercase().contains(&needle_lower) {
                    out.push((rec.path.clone(), sym));
                }
            }
        }
        out.sort_by(|a, b| {
            let exact = |n: &str| n.eq_ignore_ascii_case(needle);
            exact(&b.1.name)
                .cmp(&exact(&a.1.name))
                .then_with(|| a.1.name.len().cmp(&b.1.name.len()))
                .then_with(|| a.1.name.cmp(&b.1.name))
        });
        out.truncate(limit);
        out
    }

}
