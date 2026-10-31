use crate::kernel::state::Editor;
use crate::tests::helpers;

fn focused_key(ed: &Editor) -> usize {
    ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .unwrap_or(0)
}

#[test]
fn ts_trees_and_languages_stored_on_editor() {
    let mut ed = helpers::make_editor_with_buffer("fn main() {}");
    let key = focused_key(&ed);

    ed.ts_languages.insert(key, "rust".to_string());

    // The tree cache only holds real parse trees; without a grammar loaded
    // there is nothing cached yet (docs/core-redesign.md §7.2).
    assert!(!ed.ts_trees.contains_key(&key));
    assert_eq!(ed.ts_languages.get(&key).unwrap(), "rust");
}

#[test]
fn ts_has_tree_returns_false_for_missing() {
    let ed = helpers::make_editor();
    assert!(!ed.ts_trees.contains_key(&999));
}

#[test]
fn ts_language_set_on_valid_buffer() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    let key = focused_key(&ed);
    ed.ts_languages.insert(key, "janet".to_string());
    assert_eq!(ed.ts_languages.get(&key).unwrap(), "janet");
}

// ── Scheduled tree-sitter parsing (lane-backed, revision-checked) ────────

#[test]
fn scheduled_parse_is_a_noop_without_a_registered_grammar() {
    let mut ed = crate::tests::helpers::make_editor();
    let key = ed.create_buffer_from_str("x.rs", "fn a() {}");
    ed.ts_languages.insert(key, "rust".to_string());
    // No grammar in the map: the lane job's parse returns None and the
    // handoff store stays empty — never a panic, never a stale tree.
    crate::kernel::semantic::ts_schedule::schedule_tree_update(&mut ed, key);
    assert!(!ed.ts_trees.contains_key(&key));
}

#[test]
fn dropping_a_buffer_clears_tree_sitter_state() {
    let mut ed = helpers::make_editor_with_buffer("fn main() {}");
    let key = focused_key(&ed);
    ed.ts_languages.insert(key, "rust".to_string());

    // The slab reuses slots: a stale language entry would make the next
    // buffer in this slot skip ts/set-language (ts/has-tree? only checks
    // membership) and parse with the previous buffer's grammar.
    ed.drop_buffer(key);
    assert!(!ed.ts_languages.contains_key(&key));
    assert!(!ed.ts_trees.contains_key(&key));
}

#[test]
fn highlight_layer_writes_reach_every_view_fork() {
    let mut ed = helpers::make_editor_with_buffer("abc def");
    let key = focused_key(&ed);
    // A second window fork shows the same buffer; with a single-fork
    // write, unordered HashMap pick decided which fork got the layer and
    // the render could read the other one.
    let fork = ed.views.get(&key).cloned().unwrap();
    ed.window_views.insert((kernel::state::id::WindowId(4242), key), fork);

    ed.for_each_view_of_buffer_mut(key, |view| {
        view.set_highlights_layer("syntax", vec![(0, 3, "keyword-face".to_string())])
    });

    assert_eq!(
        ed.window_views[&(kernel::state::id::WindowId(4242), key)]
            .highlight_layers
            .get("syntax"),
        Some(&vec![(0, 3, "keyword-face".to_string())]),
        "the forked window must see the layer"
    );
    assert!(
        ed.views[&key].highlight_layers.contains_key("syntax"),
        "the default view must see the layer"
    );
}
