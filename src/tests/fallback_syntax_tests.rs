//! Fallback syntax highlighting — ownership, incremental cache, and the
//! canonical change-publication hook, exercised end-to-end without Janet.

use crate::kernel::semantic::fallback_syntax;
use crate::tests::helpers;

fn setup_rust_buffer(ed: &mut crate::kernel::state::Editor, text: &str) -> usize {
    ed.fallback_syntax.define_rules(
        "rust",
        vec!["fn".to_string(), "let".to_string()],
        Some("//".to_string()),
    );
    ed.fallback_syntax.map_extension("rs", "rust");
    let key = ed.create_buffer("t.rs");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed.buffers.get(key).unwrap().lock().unwrap().path = Some("/t/main.rs".to_string());
    if !text.is_empty() {
        let view = ed.views.get_mut(&key).unwrap();
        view.insert(0, text);
    let _ = view;
        ed.publish_buffer_changes();
    }
    key
}

fn layer(ed: &crate::kernel::state::Editor, key: usize) -> Vec<(usize, usize, String)> {
    ed.views
        .get(&key)
        .unwrap()
        .highlight_layers
        .get("syntax")
        .cloned()
        .unwrap_or_default()
}

#[test]
fn first_change_full_scans_and_colors_the_buffer() {
    let mut ed = helpers::make_editor();
    let key = setup_rust_buffer(&mut ed, "");
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(0, "fn main() { let x = 1; }\n");
    let _ = view;
    ed.publish_buffer_changes();
    let ranges = layer(&ed, key);
    assert!(
        ranges.iter().any(|&(s, _, ref f)| s == 0 && f == "keyword-face"),
        "expected 'fn' at 0 highlighted, got {ranges:?}"
    );
}

#[test]
fn ts_owned_mode_stands_down_and_takes_over_deterministically() {
    let mut ed = helpers::make_editor();
    let key = setup_rust_buffer(&mut ed, "fn main() {}\n");

    // Ownership is state, not handler order: set it before any edit runs.
    fallback_syntax::set_ts_mode(&mut ed, "fundamental", true);
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(view.cursor_offset(), "// x\n");
    let _ = view;
    ed.publish_buffer_changes();
    assert!(
        layer(&ed, key).is_empty(),
        "fallback must not write a layer the tree-sitter lane owns"
    );

    // Taking the layer back returns it to the fallback on the next change.
    fallback_syntax::set_ts_mode(&mut ed, "fundamental", false);
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(view.cursor_offset(), "fn two() {}\n");
    let _ = view;
    ed.publish_buffer_changes();
    assert!(!layer(&ed, key).is_empty(), "fallback owns again after the flip");
}

#[test]
fn ownership_flip_clears_stale_fallback_ranges_immediately() {
    let mut ed = helpers::make_editor();
    let key = setup_rust_buffer(&mut ed, "fn main() {}\n");
    fallback_syntax::force_highlight(&mut ed, key);
    assert!(!layer(&ed, key).is_empty());

    fallback_syntax::set_ts_mode(&mut ed, "fundamental", true);
    assert!(
        layer(&ed, key).is_empty(),
        "a flip to tree-sitter must clear fallback ranges at once"
    );
}

#[test]
fn incremental_splice_matches_full_rescan() {
    let mut ed = helpers::make_editor();
    let key = setup_rust_buffer(
        &mut ed,
        "let a = 1;\n// comment line\nlet b = \"str two\";\nfn c() { return 3; }\n",
    );
    fallback_syntax::force_highlight(&mut ed, key);

    // Two edits through the canonical publication path.
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(view.cursor_offset(), "x");
    let _ = view;
    ed.publish_buffer_changes();
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(4, "let z = 9;\n");
    let _ = view;
    ed.publish_buffer_changes();
    let after_dirty = layer(&ed, key);

    fallback_syntax::force_highlight(&mut ed, key);
    let after_full = layer(&ed, key);
    assert_eq!(
        after_dirty, after_full,
        "incrementally spliced ranges must equal a full rescan"
    );
}

#[test]
fn rule_changes_invalidate_cached_ranges() {
    let mut ed = helpers::make_editor();
    let key = setup_rust_buffer(&mut ed, "fn alpha() {}\n");
    fallback_syntax::force_highlight(&mut ed, key);
    assert!(!layer(&ed, key).is_empty());

    // Re-registration replaces the keyword set; the next scan must follow.
    ed.fallback_syntax.define_rules("rust", vec!["beta".to_string()], None);
    fallback_syntax::force_highlight(&mut ed, key);
    let ranges = layer(&ed, key);
    assert!(
        !ranges.iter().any(|(_, _, f)| f == "keyword-face"),
        "old keywords must stop highlighting: {ranges:?}"
    );

    let view = ed.views.get_mut(&key).unwrap();
    view.insert(view.cursor_offset(), "beta()\n");
    let _ = view;
    ed.publish_buffer_changes();
    assert!(
        layer(&ed, key).iter().any(|(_, _, f)| f == "keyword-face"),
        "new keywords must highlight on the incremental path"
    );
}

#[test]
fn whole_buffer_replacement_stays_correct() {
    let mut ed = helpers::make_editor();
    let key = setup_rust_buffer(&mut ed, "fn one() {}\nfn two() {}\n");
    fallback_syntax::force_highlight(&mut ed, key);

    let len = ed.buffers.get(key).unwrap().lock().unwrap().len();
    let view = ed.views.get_mut(&key).unwrap();
    view.delete(0, len);
    view.insert(0, "let replaced = true;\n");
    let _ = view;
    ed.publish_buffer_changes();

    let expected = {
        // A full rescan of the same text is the oracle.
        let mut probe = helpers::make_editor();
        let pk = setup_rust_buffer(&mut probe, "let replaced = true;\n");
        fallback_syntax::force_highlight(&mut probe, pk);
        layer(&probe, pk)
    };
    assert_eq!(layer(&ed, key), expected);
}

#[test]
fn language_path_wins_over_buffer_path() {
    let mut ed = helpers::make_editor();
    ed.fallback_syntax
        .define_rules("janet", vec!["defn".to_string()], Some("#".to_string()));
    ed.fallback_syntax.map_extension("janet", "janet");
    let key = ed.create_buffer_from_str("diff-pane", "defn f []\n");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    {
        let mut buf = ed.buffers.get(key).unwrap().lock().unwrap();
        buf.path = Some("/plain/noext".to_string());
        buf.language_path = Some("/src/rules.janet".to_string());
    }
    fallback_syntax::force_highlight(&mut ed, key);
    assert!(
        layer(&ed, key).iter().any(|&(s, _, _)| s == 0),
        "the language path's extension must drive the rules"
    );
}

#[test]
fn language_change_rescans_under_new_rules() {
    let mut ed = helpers::make_editor();
    let key = setup_rust_buffer(&mut ed, "fn main() {}\n");
    fallback_syntax::force_highlight(&mut ed, key);

    // The path moves to an unmapped language: the fallback owns nothing.
    ed.buffers.get(key).unwrap().lock().unwrap().path = Some("/t/other.txt".to_string());
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(view.cursor_offset(), "fn more()\n");
    let _ = view;
    ed.publish_buffer_changes();
    assert!(
        layer(&ed, key).is_empty(),
        "an unmapped language must not keep stale rust ranges"
    );
}
