//! Indent and insert-mode line commands (TAB-path ports from Janet).

use crate::kernel::command;
use crate::kernel::command::args::ArgValue;
use crate::tests::helpers;

fn set_cursor(ed: &mut crate::kernel::state::Editor, offset: usize) {
    let key = helpers::focused_key(ed);
    ed.views.get_mut(&key).unwrap().set_cursor(offset);
}

fn run_with_count(ed: &mut crate::kernel::state::Editor, cmd: &str, count: i64) {
    let mut args = std::collections::HashMap::new();
    args.insert("count".to_string(), ArgValue::Integer(count));
    command::execute_command(ed, cmd, &args).unwrap();
}

#[test]
fn indent_line_inserts_four_spaces_at_line_start() {
    let mut ed = helpers::make_editor_with_buffer("alpha\nbeta");
    helpers::run(&mut ed, "indent-line");
    assert_eq!(helpers::buf_text(&ed), "    alpha\nbeta");
}

#[test]
fn indent_line_count_repeats_the_indent() {
    let mut ed = helpers::make_editor_with_buffer("alpha");
    run_with_count(&mut ed, "indent-line", 2);
    assert_eq!(helpers::buf_text(&ed), "        alpha");
}

#[test]
fn deindent_line_removes_up_to_four_leading_spaces() {
    let mut ed = helpers::make_editor_with_buffer("    alpha\nbeta");
    helpers::run(&mut ed, "deindent-line");
    assert_eq!(helpers::buf_text(&ed), "alpha\nbeta");
}

#[test]
fn deindent_line_stops_at_non_whitespace() {
    let mut ed = helpers::make_editor_with_buffer("  ab alpha");
    helpers::run(&mut ed, "deindent-line");
    assert_eq!(helpers::buf_text(&ed), "ab alpha");
}

#[test]
fn deindent_line_removes_a_tab() {
    let mut ed = helpers::make_editor_with_buffer("\tabc");
    helpers::run(&mut ed, "deindent-line");
    assert_eq!(helpers::buf_text(&ed), "abc");
}

#[test]
fn delete_to_eol_removes_from_cursor_to_end_of_line() {
    let mut ed = helpers::make_editor_with_buffer("alpha beta\ngamma");
    set_cursor(&mut ed, 5);
    helpers::run(&mut ed, "delete-to-eol");
    assert_eq!(helpers::buf_text(&ed), "alpha\ngamma");
}

#[test]
fn delete_to_eol_on_last_line_stops_at_buffer_end() {
    let mut ed = helpers::make_editor_with_buffer("alpha\nbeta gamma");
    set_cursor(&mut ed, 10);
    helpers::run(&mut ed, "delete-to-eol");
    assert_eq!(helpers::buf_text(&ed), "alpha\nbeta");
}

#[test]
fn delete_to_eol_keeps_the_newline() {
    let mut ed = helpers::make_editor_with_buffer("alpha\n");
    set_cursor(&mut ed, 2);
    helpers::run(&mut ed, "delete-to-eol");
    assert_eq!(helpers::buf_text(&ed), "al\n");
}

#[test]
fn delete_to_bol_removes_from_line_start_to_cursor() {
    let mut ed = helpers::make_editor_with_buffer("alpha beta");
    set_cursor(&mut ed, 5);
    helpers::run(&mut ed, "delete-to-bol");
    assert_eq!(helpers::buf_text(&ed), " beta");
}

#[test]
fn change_to_eol_uses_the_native_delete_to_eol() {
    // change-to-eol dispatches to delete-to-eol; before the port that was a
    // round-trip through a Janet command.
    let mut ed = helpers::make_editor_with_buffer("alpha beta\ngamma");
    set_cursor(&mut ed, 5);
    helpers::run(&mut ed, "change-to-eol");
    assert_eq!(helpers::buf_text(&ed), "alpha\ngamma");
    assert_eq!(ed.editor_mode.name, "insert");
}
