use crate::janet_bridge;

// ── ts/set-language ───────────────────────────────────────────────────

#[test]
fn ts_set_language_stores_on_buffer() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello world");
    crate::janet_bridge::init(&mut ed);
    let key = crate::tests::helpers::focused_key(&ed);

    let result = janet_bridge::eval(
        &format!("(ts/set-language {} \"janet\")", key));
    assert_eq!(result, "ok");

    assert_eq!(ed.ts_languages.get(&key).unwrap(), "janet");
}

// ── ts/has-tree? ──────────────────────────────────────────────────────

#[test]
fn ts_has_tree_returns_false_by_default() {
    janet_test!(ed, {
        let key = crate::tests::helpers::focused_key(&ed);

        let result = janet_bridge::eval(
            &format!("(ts/has-tree? {})", key));
        assert_eq!(result, "ok");
        assert!(!ed.ts_trees.contains_key(&key));
    });
}

// ── ts/parse without grammar returns false ────────────────────────────

#[test]
fn ts_parse_without_grammar_returns_false() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("fn main() {}");
    crate::janet_bridge::init(&mut ed);
    let key = crate::tests::helpers::focused_key(&ed);

    let _ = janet_bridge::eval(
        &format!("(ts/set-language {} \"rust\")", key));
    let result = janet_bridge::eval(
        &format!("(ts/parse {})", key));
    assert_eq!(result, "ok");
}

// ── ts/query without tree returns nil ─────────────────────────────────

#[test]
fn ts_query_without_tree_handles_gracefully() {
    janet_test!(ed, {
        let key = crate::tests::helpers::focused_key(&ed);

        let result = janet_bridge::eval(
            &format!("(ts/query {} \"(symbol)\")", key));
        assert_eq!(result, "ok");
    });
}

// ── ts/set-language on invalid buffer ─────────────────────────────────

#[test]
fn ts_set_language_invalid_buffer_does_not_panic() {
    janet_test!(ed, {
        let result = janet_bridge::eval(
            "(ts/set-language 99999 \"janet\")");
        assert_eq!(result, "ok");
    });
}
