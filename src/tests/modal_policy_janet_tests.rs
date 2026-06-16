use crate::janet_bridge;
use crate::input::dispatch_key;

fn text(ed: &crate::state::Editor) -> String {
    let slab = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid)).unwrap();
    ed.buffers.get(slab).unwrap()
        .slice(0, ed.buffers.get(slab).unwrap().len())
}

// ── input/register-operator ───────────────────────────────────────────

#[test]
fn register_operator_adds_to_registry() {
    janet_test!(ed, {
        assert!(ed.vim_operators.contains_key("d"));
        assert!(ed.vim_operators.contains_key("c"));
        assert!(ed.vim_operators.contains_key("y"));
    });
}

#[test]
fn register_operator_line_cmd_correct() {
    janet_test!(ed, {
        assert_eq!(ed.vim_operators.get("d").map(|s| s.as_str()), Some("delete-line"));
        assert_eq!(ed.vim_operators.get("y").map(|s| s.as_str()), Some("yank-line"));
    });
}

#[test]
fn clear_operators_removes_all() {
    janet_test!(ed, {
        assert!(!ed.vim_operators.is_empty());
        janet_bridge::eval("(input/clear-operators)");
        assert!(ed.vim_operators.is_empty());
    });
}

#[test]
fn get_operator_returns_line_cmd() {
    janet_test!(ed, {
        janet_bridge::eval(r#"(input/register-operator "X" "delete-line")"#);
        assert_eq!(ed.vim_operators.get("X").map(|s| s.as_str()), Some("delete-line"));
    });
}

// ── input/register-prefix ─────────────────────────────────────────────

#[test]
fn register_prefix_populates_registry() {
    janet_test!(ed, {
        assert!(ed.vim_prefixes.contains("g"));
        assert!(ed.vim_prefixes.contains("z"));
        assert!(ed.vim_prefixes.contains("ctrl-w"));
    });
}

#[test]
fn unregister_prefix_removes_key() {
    janet_test!(ed, {
        assert!(ed.vim_prefixes.contains("g"));
        janet_bridge::eval(r#"(input/unregister-prefix "g")"#);
        assert!(!ed.vim_prefixes.contains("g"));
    });
}

#[test]
fn registered_prefix_p_query() {
    janet_test!(ed, {
        assert!(ed.vim_prefixes.contains("g"));
        assert!(!ed.vim_prefixes.contains("x"));
    });
}

// ── input/register-motion ─────────────────────────────────────────────

#[test]
fn register_motion_populates_registry() {
    janet_test!(ed, {
        assert!(ed.vim_motions.contains_key("w"));
        assert!(ed.vim_motions.contains_key("h"));
        assert!(ed.vim_motions.contains_key("G"));
    });
}

#[test]
fn motion_command_correct() {
    janet_test!(ed, {
        assert_eq!(ed.vim_motions.get("w").map(|s| s.as_str()), Some("move-word-forward"));
        assert_eq!(ed.vim_motions.get("h").map(|s| s.as_str()), Some("cursor-left"));
    });
}

#[test]
fn clear_motions_removes_all() {
    janet_test!(ed, {
        assert!(!ed.vim_motions.is_empty());
        janet_bridge::eval("(input/clear-motions)");
        assert!(ed.vim_motions.is_empty());
    });
}

// ── input/register-char-capture ───────────────────────────────────────

#[test]
fn char_capture_registry_populated() {
    janet_test!(ed, {
        assert!(ed.vim_char_captures.contains_key("f"));
        assert!(ed.vim_char_captures.contains_key("r"));
        assert!(ed.vim_char_captures.contains_key("m"));
    });
}

#[test]
fn char_capture_find_forward_dispatch() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    crate::janet_bridge::init(&mut ed);
    let slab = ed.windows.focused_window().and_then(|wid| ed.windows.buffer(wid)).unwrap();
    ed.buffers.get_mut(slab).unwrap().set_cursor(0);
    dispatch_key(&mut ed, "f");
    dispatch_key(&mut ed, "l");
    let c = ed.buffers.get(slab).unwrap().cursor();
    assert_eq!(c, 2, "f+l should move cursor to 'l' at index 2");
}

// ── Two-key prefix keymap entries (replace handle_prefix) ─────────────

#[test]
fn gg_goes_to_buffer_start() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\nworld");
    crate::janet_bridge::init(&mut ed);
    let slab = ed.windows.focused_window().and_then(|wid| ed.windows.buffer(wid)).unwrap();
    ed.buffers.get_mut(slab).unwrap().set_cursor(6);
    dispatch_key(&mut ed, "g");
    dispatch_key(&mut ed, "g");
    assert_eq!(ed.buffers.get(slab).unwrap().cursor(), 0);
}

#[test]
fn zt_scrolls_to_top() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("a\nb\nc\nd");
    crate::janet_bridge::init(&mut ed);
    dispatch_key(&mut ed, "z");
    dispatch_key(&mut ed, "t");
}
