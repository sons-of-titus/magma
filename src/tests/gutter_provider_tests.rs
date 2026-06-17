//! Pure Rust tests for the GutterProvider architecture (Phase 7).

use crate::kernel::render::gutter::{FoldIcons, GutterProvider, GutterRegistry};
use crate::kernel::render::gutter_providers::{
    Breakpoints, Coverage, Diagnostics, Folding, GitSigns, JanetProvider, LineNumbers,
};
use crate::tests::helpers;

// ── GutterRegistry ────────────────────────────────────────────────────────────

#[test]
fn registry_empty_by_default() {
    let ed = helpers::make_editor();
    assert!(ed.gutter.providers.is_empty());
}

#[test]
fn add_provider_appends_to_list() {
    let mut reg = GutterRegistry::default();
    reg.add_provider(Box::new(LineNumbers));
    assert_eq!(reg.providers.len(), 1);
    assert_eq!(reg.providers[0].name(), ":line-numbers");
}

#[test]
fn add_provider_replaces_existing_by_name() {
    let mut reg = GutterRegistry::default();
    reg.add_provider(Box::new(LineNumbers));
    reg.add_provider(Box::new(LineNumbers));
    assert_eq!(reg.providers.len(), 1);
}

#[test]
fn add_provider_preserves_insertion_order() {
    let mut reg = GutterRegistry::default();
    reg.add_provider(Box::new(Breakpoints));
    reg.add_provider(Box::new(GitSigns));
    reg.add_provider(Box::new(Diagnostics));
    reg.add_provider(Box::new(LineNumbers));
    reg.add_provider(Box::new(Folding));
    let names: Vec<&str> = reg.providers.iter().map(|p| p.name()).collect();
    assert_eq!(names, vec![":breakpoints", ":git-signs", ":diagnostics", ":line-numbers", ":folding"]);
}

#[test]
fn remove_provider_by_name() {
    let mut reg = GutterRegistry::default();
    reg.add_provider(Box::new(LineNumbers));
    reg.add_provider(Box::new(Folding));
    let removed = reg.remove_provider(":line-numbers");
    assert!(removed);
    assert_eq!(reg.providers.len(), 1);
    assert_eq!(reg.providers[0].name(), ":folding");
}

#[test]
fn remove_nonexistent_provider_returns_false() {
    let mut reg = GutterRegistry::default();
    assert!(!reg.remove_provider(":nonexistent"));
}

// ── provider widths ───────────────────────────────────────────────────────────

#[test]
fn line_numbers_width_is_dynamic() {
    assert_eq!(LineNumbers.width(), 0);
}

#[test]
fn line_numbers_dynamic_width_scales_with_line_count() {
    let w10  = GutterRegistry::provider_width(&LineNumbers, 10);
    let w100 = GutterRegistry::provider_width(&LineNumbers, 100);
    assert!(w100 > w10);
}

#[test]
fn folding_width_is_one() {
    assert_eq!(Folding.width(), 1);
}

#[test]
fn git_signs_width_is_one() {
    assert_eq!(GitSigns.width(), 1);
}

#[test]
fn diagnostics_width_is_one() {
    assert_eq!(Diagnostics.width(), 1);
}

#[test]
fn breakpoints_width_is_two() {
    assert_eq!(Breakpoints.width(), 2);
}

#[test]
fn coverage_width_is_one() {
    assert_eq!(Coverage.width(), 1);
}

#[test]
fn total_width_sums_all_providers() {
    let mut reg = GutterRegistry::default();
    reg.add_provider(Box::new(Diagnostics));  // 1
    reg.add_provider(Box::new(Breakpoints));  // 2
    reg.add_provider(Box::new(Folding));      // 1
    // LineNumbers not added — total should be 4
    assert_eq!(reg.total_width(99), 4);
}

// ── FoldIcons defaults ────────────────────────────────────────────────────────

#[test]
fn fold_icons_have_defaults() {
    let ed = helpers::make_editor();
    assert_eq!(ed.gutter.fold_icons.open, "▾");
    assert_eq!(ed.gutter.fold_icons.closed, "▸");
    assert_eq!(ed.gutter.fold_icons.face, "fold-face");
}

#[test]
fn fold_icons_can_be_changed() {
    let mut ed = helpers::make_editor();
    ed.gutter.fold_icons = FoldIcons {
        open: "v".to_string(),
        closed: ">".to_string(),
        face: "my-face".to_string(),
    };
    assert_eq!(ed.gutter.fold_icons.open, "v");
    assert_eq!(ed.gutter.fold_icons.closed, ">");
}

// ── sign_cache ────────────────────────────────────────────────────────────────

#[test]
fn sign_cache_empty_by_default() {
    let ed = helpers::make_editor();
    assert!(ed.gutter.sign_cache.is_empty());
}

#[test]
fn set_provider_cells_stores_and_retrieves_cells() {
    use crate::kernel::render::surface::Style;
    let mut reg = GutterRegistry::default();
    let style = Style::default();
    reg.set_provider_cells(":git-signs", 0, vec![(5, "▎".to_string(), style)]);
    let cell = reg.cached_cell(":git-signs", 0, 5).unwrap();
    assert_eq!(cell.text, "▎");
}

#[test]
fn set_provider_cells_replaces_existing() {
    use crate::kernel::render::surface::Style;
    let mut reg = GutterRegistry::default();
    let style = Style::default();
    reg.set_provider_cells(":git-signs", 0, vec![(3, "▎".to_string(), style)]);
    reg.set_provider_cells(":git-signs", 0, vec![(3, "M".to_string(), style)]);
    let cell = reg.cached_cell(":git-signs", 0, 3).unwrap();
    assert_eq!(cell.text, "M");
}

#[test]
fn cached_cell_returns_none_for_missing_line() {
    let reg = GutterRegistry::default();
    assert!(reg.cached_cell(":git-signs", 0, 99).is_none());
}

// ── line_number_fn ────────────────────────────────────────────────────────────

#[test]
fn line_number_fn_is_none_by_default() {
    let ed = helpers::make_editor();
    assert!(ed.gutter.line_number_fn.is_none());
}

#[test]
fn line_number_fn_can_be_set() {
    let mut ed = helpers::make_editor();
    ed.gutter.line_number_fn = Some("gutter-lnum-hybrid".to_string());
    assert_eq!(ed.gutter.line_number_fn.as_deref(), Some("gutter-lnum-hybrid"));
}

// ── JanetProvider ─────────────────────────────────────────────────────────────

#[test]
fn janet_provider_stores_name_and_fn() {
    let jp = JanetProvider {
        provider_name: "my-signs".to_string(),
        fn_name: Some("compute-signs".to_string()),
    };
    assert_eq!(jp.name(), "my-signs");
    assert_eq!(jp.fn_name.as_deref(), Some("compute-signs"));
}

#[test]
fn janet_provider_width_is_one() {
    let jp = JanetProvider { provider_name: "x".to_string(), fn_name: None };
    assert_eq!(jp.width(), 1);
}
