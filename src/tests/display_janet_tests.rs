use crate::janet_bridge;

// ── editor/set-cursor-shape ───────────────────────────────────────────

#[test]
fn editor_set_cursor_shape_block() {
    janet_test!(ed, {
        let result = janet_bridge::eval("(editor/set-cursor-shape \"block\")");
        assert_eq!(result, "ok");
        assert_eq!(ed.cursor_shape, "block");
    });
}

#[test]
fn editor_set_cursor_shape_beam() {
    janet_test!(ed, {
        let result = janet_bridge::eval("(editor/set-cursor-shape \"beam\")");
        assert_eq!(result, "ok");
        assert_eq!(ed.cursor_shape, "beam");
    });
}

#[test]
fn editor_set_cursor_shape_underline() {
    janet_test!(ed, {
        let result = janet_bridge::eval("(editor/set-cursor-shape \"underline\")");
        assert_eq!(result, "ok");
        assert_eq!(ed.cursor_shape, "underline");
    });
}

#[test]
fn editor_cursor_shape_returns_current() {
    janet_test!(ed, {
        let result = janet_bridge::eval(
            "(do (editor/set-cursor-shape \"beam\") (editor/cursor-shape))");
        assert_eq!(result, "ok");
        assert_eq!(ed.cursor_shape, "beam");
    });
}
