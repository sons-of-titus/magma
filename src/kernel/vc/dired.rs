//! Directory editor — dired-style buffer state and types.
//!
//! Display formatting and directory reading live in the sibling module
//! `dired_display`, which is re-exported from here for convenience.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

// Re-export display utilities so consumers can `use dired::*`.
pub use super::dired_display::*;

/// How entries are sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Name,
    Size,
    Date,
}

/// What a mark on a file means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkType {
    Delete,
    Copy,
    Move,
}

/// A single entry in a dired listing.
#[derive(Debug, Clone)]
pub struct DiredEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub perms: String,
    pub modified: String,
    pub modified_raw: u64,
}

/// Active dired session state.
#[derive(Debug)]
pub struct DiredState {
    pub dir: PathBuf,
    pub entries: Vec<DiredEntry>,
    pub marks: HashMap<String, MarkType>,
    pub buf_key: Option<usize>,
    pub active: bool,
    pub sort_field: SortField,
    pub sort_reverse: bool,
    pub show_hidden: bool,
    pub filter_pattern: Option<String>,
}

impl Default for DiredState {
    fn default() -> Self {
        DiredState {
            dir: PathBuf::new(),
            entries: Vec::new(),
            marks: HashMap::new(),
            buf_key: None,
            active: false,
            sort_field: SortField::Name,
            sort_reverse: false,
            show_hidden: false,
            filter_pattern: None,
        }
    }
}

impl DiredState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the entry for display line `line` (0-indexed), or `None`.
    pub fn entry_at_line(&self, line: usize) -> Option<&DiredEntry> {
        let idx = line.checked_sub(HEADER_LINES)?;
        let view = view_entries(&self.entries, self.filter_pattern.as_deref(),
            self.show_hidden, self.sort_field, self.sort_reverse);
        view.get(idx).copied()
    }

    /// Full path for a named entry in the current directory.
    pub fn full_path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Reload entries from disk and return updated display text.
    pub fn reload(&mut self) -> Result<String, String> {
        self.entries = read_dir(&self.dir.clone())?;
        Ok(build_display(&self.dir, &self.entries, &self.marks,
            self.sort_field, self.sort_reverse,
            self.filter_pattern.as_deref(), self.show_hidden))
    }

    /// Get visible entry at line index within the filtered/sorted view.
    pub fn visible_entries(&self) -> Vec<&DiredEntry> {
        view_entries(&self.entries, self.filter_pattern.as_deref(),
            self.show_hidden, self.sort_field, self.sort_reverse)
    }

    /// Toggle a mark type on a file entry.
    pub fn toggle_mark(&mut self, name: &str, mt: MarkType) {
        if name == "." || name == ".." { return; }
        if self.marks.get(name) == Some(&mt) {
            self.marks.remove(name);
        } else {
            self.marks.insert(name.to_string(), mt);
        }
    }

    /// Count marks of each type.
    pub fn mark_counts(&self) -> Vec<(MarkType, usize)> {
        let mut counts: HashMap<MarkType, usize> = HashMap::new();
        for (_, mt) in &self.marks {
            *counts.entry(*mt).or_default() += 1;
        }
        vec![
            (MarkType::Delete, counts.get(&MarkType::Delete).copied().unwrap_or(0)),
            (MarkType::Copy, counts.get(&MarkType::Copy).copied().unwrap_or(0)),
            (MarkType::Move, counts.get(&MarkType::Move).copied().unwrap_or(0)),
        ]
    }

    /// Invert marks: mark all unmarked entries, unmark all marked.
    pub fn invert_marks(&mut self) {
        let view: Vec<&DiredEntry> = self.visible_entries();
        let mut new_marks: HashMap<String, MarkType> = HashMap::new();
        for entry in &view {
            if entry.name == "." || entry.name == ".." { continue; }
            if !self.marks.contains_key(&entry.name) {
                new_marks.insert(entry.name.clone(), MarkType::Delete);
            }
        }
        self.marks = new_marks;
    }
}
