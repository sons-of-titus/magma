//! Tests for the unified command surface: every registered command (Rust or
//! Janet) is listed for the palette, and `execute_command_line` routes
//! `"name args…"` input to both command flavors.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::kernel::command::{args::ArgSpec, execute_command_line};
use crate::tests::helpers::make_editor;

fn capture_command(ed: &mut crate::kernel::state::Editor, specs: Vec<ArgSpec>) -> Arc<Mutex<Option<HashMap<String, String>>>> {
    let captured = Arc::new(Mutex::new(None));
    let sink = captured.clone();
    ed.commands.register("surface-capture", "test command", specs, Box::new(move |_ed, args| {
        let flat: HashMap<String, String> = args.iter()
            .filter_map(|(k, v)| v.as_string().map(|s| (k.clone(), s.to_string())))
            .collect();
        *sink.lock().unwrap_or_else(|e| e.into_inner()) = Some(flat);
        Ok(())
    }));
    captured
}

#[test]
fn execute_command_line_splits_name_from_rest() {
    let mut ed = make_editor();
    let specs = vec![ArgSpec::new("body", crate::kernel::command::args::ArgType::String)];
    let captured = capture_command(&mut ed, specs);

    execute_command_line(&mut ed, "surface-capture hello world").unwrap();
    let got = captured.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap();
    assert_eq!(got.get("body").map(String::as_str), Some("hello world"),
        "everything after the first token is the argument");
}

#[test]
fn execute_command_line_maps_positional_specs() {
    let mut ed = make_editor();
    let specs = vec![
        ArgSpec::new("first", crate::kernel::command::args::ArgType::String),
        ArgSpec::new("second", crate::kernel::command::args::ArgType::String),
    ];
    let captured = capture_command(&mut ed, specs);

    execute_command_line(&mut ed, "surface-capture one two").unwrap();
    let got = captured.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap();
    assert_eq!(got.get("first").map(String::as_str), Some("one"));
    assert_eq!(got.get("second").map(String::as_str), Some("two"));
}

#[test]
fn execute_command_line_missing_required_arg_errors() {
    let mut ed = make_editor();
    let specs = vec![ArgSpec::new("body", crate::kernel::command::args::ArgType::String)];
    let _captured = capture_command(&mut ed, specs);

    let result = execute_command_line(&mut ed, "surface-capture");
    assert!(result.is_err(), "required argument missing without default");
}

#[test]
fn execute_command_line_without_args_runs_bare() {
    let mut ed = make_editor();
    let captured = capture_command(&mut ed, vec![]);

    execute_command_line(&mut ed, "surface-capture").unwrap();
    let got = captured.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap();
    assert!(got.is_empty());
}

#[test]
fn unknown_command_name_errors() {
    let mut ed = make_editor();
    let result = execute_command_line(&mut ed, "no-such-command-xyz");
    assert!(result.is_err());
}

#[test]
fn palette_listing_contains_registered_commands_with_metadata() {
    let mut ed = make_editor();
    ed.commands.register("zz-surface-pure", "pure test command", vec![],
        Box::new(|_ed, _args| Ok(())));

    let listed = ed.commands.list_detailed();
    let entry = listed.iter().find(|(name, _, _)| name == "zz-surface-pure");
    assert!(entry.is_some(), "registered commands appear in the palette listing");
    assert_eq!(entry.unwrap().1, "pure test command");
}
