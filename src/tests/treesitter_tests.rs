use crate::state::Editor;
use crate::tests::helpers;

fn focused_key(ed: &Editor) -> usize {
    ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap_or(0)
}

#[test]
fn ts_trees_and_languages_stored_on_editor() {
    let mut ed = helpers::make_editor_with_buffer("fn main() {}");
    let key = focused_key(&ed);

    ed.ts_languages.insert(key, "rust".to_string());
    ed.ts_trees.insert(key, vec![1, 2, 3]);

    assert!(ed.ts_trees.contains_key(&key));
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
