//! Built-in GutterProvider implementations — Phase 7.
//!
//! `LineNumbers`, `Folding`, `GitSigns`, `Diagnostics`, `Breakpoints`,
//! `Coverage`, and `JanetProvider`.

use crate::kernel::text_engine::Buffer;
use crate::kernel::render::surface::Style;
use crate::kernel::render::gutter::{GutterCell, GutterCtx, GutterProvider, GutterRegistry};
use crate::kernel::render::frame::bool_option;

// ── LineNumbers ───────────────────────────────────────────────────────────────

/// Renders absolute or relative line numbers.  Width is dynamic (returns 0).
pub struct LineNumbers;

impl GutterProvider for LineNumbers {
    fn name(&self) -> &str { ":line-numbers" }
    fn width(&self) -> usize { 0 }

    fn render(&self, line: usize, ctx: &GutterCtx) -> Option<GutterCell> {
        let show_rel = bool_option(ctx.editor, "relativenumber");
        let display_num = if show_rel && line != ctx.cursor_line {
            line.abs_diff(ctx.cursor_line)
        } else {
            line + 1
        };
        let col_w = GutterRegistry::provider_width(self, ctx.line_count);
        let text = format!("{:>width$} ", display_num, width = col_w.saturating_sub(1));
        let (fg, bg) = if line == ctx.cursor_line {
            (ctx.editor.theme_color("line-num-current"), ctx.editor.theme_color("line-num-bg"))
        } else {
            (ctx.editor.theme_color("line-num"), ctx.editor.theme_color("line-num-bg"))
        };
        Some(GutterCell { text, style: Style { fg, bg, ..Default::default() } })
    }
}

// ── Folding ───────────────────────────────────────────────────────────────────

/// Renders fold-open / fold-closed icons.  Width is 1.
pub struct Folding;

impl GutterProvider for Folding {
    fn name(&self) -> &str { ":folding" }
    fn width(&self) -> usize { 1 }

    fn render(&self, _line: usize, ctx: &GutterCtx) -> Option<GutterCell> {
        let fold_icons = &ctx.editor.gutter.fold_icons;
        let icon = if ctx.is_fold_start {
            fold_icons.closed.as_str()
        } else {
            " "
        };
        let style = ctx.editor.resolve_face_style(&fold_icons.face)
            .unwrap_or_else(|| Style {
                fg: ctx.editor.theme_color("line-num"),
                bg: ctx.editor.theme_color("line-num-bg"),
                ..Default::default()
            });
        Some(GutterCell { text: icon.to_string(), style })
    }
}

// ── GitSigns ──────────────────────────────────────────────────────────────────

/// Renders VC change indicators.  Cells are populated via `gutter/provider-update`
/// from Janet-side scripts (e.g. `vcs_gutter.janet`).
pub struct GitSigns;

impl GutterProvider for GitSigns {
    fn name(&self) -> &str { ":git-signs" }
    fn width(&self) -> usize { 1 }

    fn render(&self, line: usize, ctx: &GutterCtx) -> Option<GutterCell> {
        ctx.editor.gutter.cached_cell(":git-signs", ctx.buf_id, line)
    }

    fn update(&mut self, buf_id: usize, _buffer: &Buffer) {
        let _ = buf_id;
        // Cache is managed externally via gutter/provider-update.
    }
}

// ── Diagnostics ───────────────────────────────────────────────────────────────

/// Renders a single-char diagnostic severity indicator (E/W) for lines that
/// carry LSP diagnostics.  Reads from the pre-computed `ctx.diag_lines` map.
pub struct Diagnostics;

impl GutterProvider for Diagnostics {
    fn name(&self) -> &str { ":diagnostics" }
    fn width(&self) -> usize { 1 }

    fn render(&self, line: usize, ctx: &GutterCtx) -> Option<GutterCell> {
        let sev = ctx.diag_lines.get(&line)?;
        let (bg_r, bg_g, bg_b) = ctx.editor.theme_color("line-num-bg");
        let fg = if *sev == 'E' {
            ctx.editor.theme_color("diag-error")
        } else {
            ctx.editor.theme_color("diag-warn")
        };
        Some(GutterCell {
            text: sev.to_string(),
            style: Style { fg, bg: (bg_r, bg_g, bg_b), bold: true, ..Default::default() },
        })
    }
}

// ── Breakpoints ───────────────────────────────────────────────────────────────

/// Renders a breakpoint indicator (●) for lines that have a DAP breakpoint.
/// Reads live from `ctx.editor.debug.breakpoints`.
pub struct Breakpoints;

impl GutterProvider for Breakpoints {
    fn name(&self) -> &str { ":breakpoints" }
    fn width(&self) -> usize { 2 }

    fn render(&self, line: usize, ctx: &GutterCtx) -> Option<GutterCell> {
        let path = ctx.buf_path?;
        let bps = ctx.editor.debug.breakpoints_for(path);
        // line in GutterCtx is 0-based; DAP breakpoints store 1-based lines.
        let has_bp = bps.iter().any(|bp| bp.line == line + 1);
        if !has_bp { return None; }
        let style = ctx.editor.resolve_face_style("gutter-breakpoint")
            .unwrap_or_else(|| Style {
                fg: ctx.editor.theme_color("diag-error"),
                bg: ctx.editor.theme_color("line-num-bg"),
                bold: true,
                ..Default::default()
            });
        Some(GutterCell { text: "● ".to_string(), style })
    }
}

// ── Coverage ──────────────────────────────────────────────────────────────────

/// Placeholder for a future code-coverage provider.  Always renders blank.
pub struct Coverage;

impl GutterProvider for Coverage {
    fn name(&self) -> &str { ":coverage" }
    fn width(&self) -> usize { 1 }
    fn render(&self, _line: usize, _ctx: &GutterCtx) -> Option<GutterCell> { None }
}

// ── JanetProvider ─────────────────────────────────────────────────────────────

/// A Janet-defined gutter provider.  Cells are populated via
/// `gutter/provider-update` from Janet; `render()` looks up the cached cell.
pub struct JanetProvider {
    pub provider_name: String,
    pub fn_name: Option<String>,
}

impl GutterProvider for JanetProvider {
    fn name(&self) -> &str { &self.provider_name }
    fn width(&self) -> usize { 1 }

    fn render(&self, line: usize, ctx: &GutterCtx) -> Option<GutterCell> {
        ctx.editor.gutter.cached_cell(&self.provider_name, ctx.buf_id, line)
    }

    fn update(&mut self, buf_id: usize, _buffer: &Buffer) {
        let _ = buf_id;
        // Cache is managed externally via gutter/provider-update.
    }
}
