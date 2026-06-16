use crate::state::Editor;
use crate::tests::helpers;

#[test]
fn default_cursor_shape_is_block() {
    let ed = helpers::make_editor();
    assert_eq!(ed.cursor_shape, "block");
}

#[test]
fn cursor_shape_can_be_set_to_beam() {
    let mut ed = helpers::make_editor();
    ed.cursor_shape = "beam".to_string();
    assert_eq!(ed.cursor_shape, "beam");
}

#[test]
fn cursor_shape_can_be_set_to_underline() {
    let mut ed = helpers::make_editor();
    ed.cursor_shape = "underline".to_string();
    assert_eq!(ed.cursor_shape, "underline");
}

#[test]
fn integer_option_returns_default_when_missing() {
    let ed = helpers::make_editor();
    let val = crate::render::frame::int_option(&ed, "colorcolumn", 0);
    assert_eq!(val, 0);
}

#[test]
fn integer_option_parses_value() {
    let mut ed = helpers::make_editor();
    ed.options.insert("colorcolumn".to_string(), "80".to_string());
    let val = crate::render::frame::int_option(&ed, "colorcolumn", 0);
    assert_eq!(val, 80);
}

#[test]
fn bool_option_returns_false_when_missing() {
    let ed = helpers::make_editor();
    let val = crate::render::frame::bool_option(&ed, "number");
    assert!(!val);
}

#[test]
fn bool_option_returns_true_when_set() {
    let mut ed = helpers::make_editor();
    ed.options.insert("number".to_string(), "true".to_string());
    let val = crate::render::frame::bool_option(&ed, "number");
    assert!(val);
}
