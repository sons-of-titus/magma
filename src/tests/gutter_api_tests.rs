//! Pure Rust tests for the named-column gutter system (Sprint 11c).

use crate::state::{Editor, GutterColumn, GutterSign, FoldIcons};
use crate::tests::helpers;

// ── GutterColumn ─────────────────────────────────────────────────────────────

#[test]
fn gutter_columns_empty_by_default() {
    let ed = helpers::make_editor();
    assert!(ed.gutter.columns.is_empty());
}

#[test]
fn gutter_column_can_be_added() {
    let mut ed = helpers::make_editor();
    ed.gutter.columns.push(GutterColumn {
        name: ":line-numbers".to_string(),
        width: 0,
        visible: true,
        face: "gutter-bg".to_string(),
    });
    assert_eq!(ed.gutter.columns.len(), 1);
    assert_eq!(ed.gutter.columns[0].name, ":line-numbers");
    assert_eq!(ed.gutter.columns[0].width, 0);
}

#[test]
fn gutter_column_insertion_order_preserved() {
    let mut ed = helpers::make_editor();
    for name in &[":breakpoints", ":vcs", ":diagnostics", ":line-numbers", ":folding"] {
        ed.gutter.columns.push(GutterColumn {
            name: name.to_string(),
            width: 1,
            visible: true,
            face: "gutter-bg".to_string(),
        });
    }
    let names: Vec<&str> = ed.gutter.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec![":breakpoints", ":vcs", ":diagnostics", ":line-numbers", ":folding"]);
}

#[test]
fn gutter_column_visibility_toggle() {
    let mut ed = helpers::make_editor();
    ed.gutter.columns.push(GutterColumn {
        name: ":breakpoints".to_string(),
        width: 2,
        visible: true,
        face: "gutter-bg".to_string(),
    });
    ed.gutter.columns[0].visible = false;
    assert!(!ed.gutter.columns[0].visible);
    ed.gutter.columns[0].visible = true;
    assert!(ed.gutter.columns[0].visible);
}

// ── FoldIcons ─────────────────────────────────────────────────────────────────

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

// ── GutterSign text field ─────────────────────────────────────────────────────

#[test]
fn gutter_sign_text_field_stores_string() {
    let sign = GutterSign {
        face: "gutter-breakpoint".to_string(),
        text: "● ".to_string(),
        priority: 100,
    };
    assert_eq!(sign.text, "● ");
}

#[test]
fn gutter_sign_multi_char_text_allowed() {
    let sign = GutterSign {
        face: "gutter-vcs-changed".to_string(),
        text: "▎".to_string(),
        priority: 50,
    };
    assert_eq!(sign.text, "▎");
}

// ── GutterColumnSigns ────────────────────────────────────────────────────────

#[test]
fn gutter_column_signs_empty_by_default() {
    let ed = helpers::make_editor();
    assert!(ed.gutter.column_signs.is_empty());
}

#[test]
fn gutter_column_sign_can_be_set() {
    let mut ed = helpers::make_editor();
    let key = (":breakpoints".to_string(), 0usize);
    ed.gutter.column_signs
        .entry(key)
        .or_default()
        .entry(5)
        .or_default()
        .push(GutterSign { face: "gutter-breakpoint".to_string(), text: "● ".to_string(), priority: 100 });
    assert!(ed.gutter.column_signs.contains_key(&(":breakpoints".to_string(), 0)));
    assert_eq!(ed.gutter.column_signs[&(":breakpoints".to_string(), 0)][&5].len(), 1);
}

#[test]
fn gutter_column_sign_highest_priority_wins() {
    let mut ed = helpers::make_editor();
    let key = (":diagnostics".to_string(), 0usize);
    let low = GutterSign { face: "warning-face".to_string(), text: "W".to_string(), priority: 5 };
    let high = GutterSign { face: "error-face".to_string(), text: "E".to_string(), priority: 10 };
    let signs = ed.gutter.column_signs.entry(key).or_default().entry(0).or_default();
    signs.push(low);
    signs.push(high);
    let top = ed.gutter.column_signs[&(":diagnostics".to_string(), 0)][&0]
        .iter().max_by_key(|s| s.priority).unwrap();
    assert_eq!(top.text, "E");
}

#[test]
fn gutter_column_signs_cleared_per_column() {
    let mut ed = helpers::make_editor();
    let k1 = (":vcs".to_string(), 0usize);
    let k2 = (":diagnostics".to_string(), 0usize);
    ed.gutter.column_signs.entry(k1.clone()).or_default().entry(0).or_default()
        .push(GutterSign { face: "f".to_string(), text: "▎".to_string(), priority: 1 });
    ed.gutter.column_signs.entry(k2.clone()).or_default().entry(0).or_default()
        .push(GutterSign { face: "f".to_string(), text: "E".to_string(), priority: 1 });
    ed.gutter.column_signs.remove(&k1);
    assert!(!ed.gutter.column_signs.contains_key(&k1));
    assert!(ed.gutter.column_signs.contains_key(&k2));
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
