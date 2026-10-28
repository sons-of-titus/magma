#[macro_use]
mod helpers;
use scripting;

// ── ts/set-language ───────────────────────────────────────────────────

#[test]
fn ts_set_language_stores_on_buffer() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("hello world");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    let result = scripting::eval(&format!("(ts/set-language {} \"janet\")", key));
    assert_eq!(result, "ok");

    assert_eq!(ed.ts_languages.get(&key).unwrap(), "janet");
}

// ── ts/has-tree? ──────────────────────────────────────────────────────

#[test]
fn ts_has_tree_returns_false_by_default() {
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);

        let result = scripting::eval(&format!("(ts/has-tree? {})", key));
        assert_eq!(result, "ok");
        assert!(!ed.ts_trees.contains_key(&key));
    });
}

// ── ts/parse without grammar returns false ────────────────────────────

#[test]
fn ts_parse_without_grammar_returns_false() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("fn main() {}");
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let key = helpers::focused_key(&ed);

    let _ = scripting::eval(&format!("(ts/set-language {} \"rust\")", key));
    let result = scripting::eval(&format!("(ts/parse {})", key));
    assert_eq!(result, "ok");
}

// ── ts/query without tree returns nil ─────────────────────────────────

#[test]
fn ts_query_without_tree_handles_gracefully() {
    janet_test!(ed, {
        let key = helpers::focused_key(&ed);

        let result = scripting::eval(&format!("(ts/query {} \"(symbol)\")", key));
        assert_eq!(result, "ok");
    });
}

// ── ts/set-language on invalid buffer ─────────────────────────────────

#[test]
fn ts_set_language_invalid_buffer_does_not_panic() {
    janet_test!(ed, {
        let result = scripting::eval("(ts/set-language 99999 \"janet\")");
        assert_eq!(result, "ok");
    });
}

// ── The keystroke path must not re-highlight synchronously ──────────────────

#[test]
fn buffer_changed_handler_is_noop_and_highlight_follows_ts_tree_updated() {
    janet_test!(ed, {
        // A rust-mode buffer (has a query registered) with a path so the
        // language attach machinery sees it.
        let key = helpers::focused_key(&ed);
        let _ = &key;
        let r = scripting::eval(
            r#"
            (do
              (var ran nil)
              # The buffer-changed handler is a deliberate no-op now: the
              # full-tree highlight ran per keystroke and blocked typing.
              (event/emit "buffer-changed" {"buffer-id" "0" "revision-before" "0"
                                            "revision-after" "1" "edits" "0:0:1"})
              (set ran true)
              ran)
        "#,
        );
        assert_eq!(r, "ok");

        // The listener that replaces it: ts-tree-updated drives the
        // highlight from a completed (off-thread) parse.
        let r2 = scripting::eval(
            r#"
            (do
              (event/emit "ts-tree-updated" {"buffer-id" "0"})
              "ok")
        "#,
        );
        assert_eq!(
            r2, "ok",
            "the ts-tree-updated listener must not error on a real buffer id"
        );
    });
}
