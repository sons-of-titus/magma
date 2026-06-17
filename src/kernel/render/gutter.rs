//! GutterProvider trait, GutterCtx, and GutterRegistry — Phase 7.

use std::collections::HashMap;

use crate::kernel::text_engine::Buffer;
use crate::kernel::render::surface::Style;

// ── FoldIcons ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FoldIcons {
    pub open: String,
    pub closed: String,
    pub face: String,
}

impl Default for FoldIcons {
    fn default() -> Self {
        FoldIcons {
            open: "▾".to_string(),
            closed: "▸".to_string(),
            face: "fold-face".to_string(),
        }
    }
}

// ── GutterCell ────────────────────────────────────────────────────────────────

/// The rendered output of a single gutter cell for one buffer line.
#[derive(Clone)]
pub struct GutterCell {
    pub text: String,
    pub style: Style,
}

// ── GutterCtx ─────────────────────────────────────────────────────────────────

use crate::kernel::state::Editor;

/// Context supplied to every `GutterProvider::render` call.
pub struct GutterCtx<'a> {
    pub editor: &'a Editor,
    pub buf_id: usize,
    /// Absolute path of the focused buffer (for Diagnostics / Breakpoints lookup).
    pub buf_path: Option<&'a str>,
    /// 0-based index of the line the cursor is on.
    pub cursor_line: usize,
    /// Total number of lines in the buffer.
    pub line_count: usize,
    /// Byte offset of the start of the line currently being rendered.
    pub line_start_offset: usize,
    /// True when this line is the start of a folded region.
    pub is_fold_start: bool,
    /// Pre-computed diagnostic severity for lines (0-based line → severity char).
    pub diag_lines: &'a HashMap<usize, char>,
}

// ── GutterProvider trait ──────────────────────────────────────────────────────

/// A contributor to the editor gutter.  Each provider renders one column.
pub trait GutterProvider: Send + Sync {
    fn name(&self) -> &str;

    /// Fixed column width in terminal cells.  Return 0 for dynamic sizing —
    /// `GutterRegistry::provider_width` will derive the width from the buffer's
    /// line count.
    fn width(&self) -> usize;

    /// Render the cell for `line` (0-based) in this column.
    /// Return `None` to leave the cell blank (background colour only).
    fn render(&self, line: usize, ctx: &GutterCtx) -> Option<GutterCell>;

    /// Called when buffer state changes so providers can refresh cached data.
    /// The default implementation is a no-op.
    fn update(&mut self, _buf_id: usize, _buffer: &Buffer) {}
}

// ── GutterRegistry ────────────────────────────────────────────────────────────

pub struct GutterRegistry {
    pub providers: Vec<Box<dyn GutterProvider>>,
    pub fold_icons: FoldIcons,
    pub line_number_fn: Option<String>,
    /// Pre-computed cell cache for data-driven providers (JanetProvider, GitSigns).
    /// Key: (provider_name, buf_id, line_index) → (text, style).
    /// Populated by `gutter/provider-update` from Janet side.
    pub sign_cache: HashMap<(String, usize, usize), (String, Style)>,
}

impl Default for GutterRegistry {
    fn default() -> Self {
        GutterRegistry {
            providers: Vec::new(),
            fold_icons: FoldIcons::default(),
            line_number_fn: None,
            sign_cache: HashMap::new(),
        }
    }
}

impl GutterRegistry {
    /// Resolve the display width of a provider, handling dynamic sizing.
    pub fn provider_width(provider: &dyn GutterProvider, line_count: usize) -> usize {
        let w = provider.width();
        if w == 0 {
            (line_count.max(1).ilog10() as usize + 1).max(2) + 1
        } else {
            w
        }
    }

    /// Total gutter width in terminal cells (sum of all provider widths).
    pub fn total_width(&self, line_count: usize) -> usize {
        self.providers.iter()
            .map(|p| Self::provider_width(p.as_ref(), line_count))
            .sum()
    }

    /// Add or replace a provider by name.
    pub fn add_provider(&mut self, provider: Box<dyn GutterProvider>) {
        if let Some(pos) = self.providers.iter().position(|p| p.name() == provider.name()) {
            self.providers[pos] = provider;
        } else {
            self.providers.push(provider);
        }
    }

    /// Remove a provider by name.  Returns `true` if one was removed.
    pub fn remove_provider(&mut self, name: &str) -> bool {
        if let Some(pos) = self.providers.iter().position(|p| p.name() == name) {
            self.providers.remove(pos);
            true
        } else {
            false
        }
    }

    /// Notify all providers that a buffer has changed.
    pub fn refresh(&mut self, buf_id: usize, buffer: &Buffer) {
        for p in &mut self.providers {
            p.update(buf_id, buffer);
        }
    }

    /// Push pre-computed cells for a named provider (used by `gutter/provider-update`).
    /// Replaces all existing cells for `(name, buf_id)`.
    pub fn set_provider_cells(
        &mut self,
        name: &str,
        buf_id: usize,
        cells: Vec<(usize, String, Style)>,
    ) {
        // Clear old entries for this (name, buf_id) pair.
        self.sign_cache.retain(|(k_name, k_buf, _), _| {
            k_name != name || *k_buf != buf_id
        });
        for (line, text, style) in cells {
            self.sign_cache.insert((name.to_string(), buf_id, line), (text, style));
        }
    }

    /// Look up a cached cell for `(name, buf_id, line)`.
    pub fn cached_cell(&self, name: &str, buf_id: usize, line: usize) -> Option<GutterCell> {
        let (text, style) = self.sign_cache.get(&(name.to_string(), buf_id, line))?;
        Some(GutterCell { text: text.clone(), style: *style })
    }
}
