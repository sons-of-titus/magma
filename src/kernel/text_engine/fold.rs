//! Buffer fold methods: add, remove, clear, query.

use super::Buffer;

impl Buffer {
    pub fn add_fold(&mut self, start: usize, end: usize) {
        self.folds.push((start, end));
        self.folds.sort_by_key(|a| a.0);
    }

    pub fn remove_fold(&mut self, start: usize, end: usize) {
        self.folds.retain(|f| f.0 != start || f.1 != end);
    }

    pub fn clear_folds(&mut self) {
        self.folds.clear();
    }

    pub fn is_folded(&self, line_start: usize, line_end: usize) -> bool {
        self.folds.iter().any(|(fs, fe)| *fs <= line_start && *fe >= line_end)
    }
}
