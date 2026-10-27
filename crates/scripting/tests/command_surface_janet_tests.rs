#[macro_use] mod helpers;
// Janet-side tests for the unified command surface: Janet-defined commands
// are visible to the palette listing and receive the raw argument from
// `execute_command_line` / `editor/run-command`.

use magma::kernel::command::execute_command_line;
use scripting;
use magma::kernel::state::MessageKind;

#[test]
fn janet_command_appears_in_palette_listing() {
    janet_test!(ed, {
        scripting::eval(r#"(command/define "surface-janet-cmd" (fn [&] nil))"#);
        let listed = ed.commands.list_detailed();
        let entry = listed.iter().find(|(name, _, _)| name == "surface-janet-cmd");
        assert!(entry.is_some(), "Janet-defined commands must be listed for the palette");
    });
}

#[test]
fn execute_command_line_passes_raw_arg_to_janet_command() {
    janet_test!(ed, {
        scripting::eval(r#"(command/define "surface-arg-cmd" (fn [arg] (editor/echo arg)))"#);
        execute_command_line(&mut ed, "surface-arg-cmd hello janet").unwrap();
        let msg = ed.status_message.as_ref().expect("command must have echoed");
        assert_eq!(msg.text, "hello janet",
            "the whole remainder after the command name is the Janet arg");
    });
}

#[test]
fn editor_run_command_delivers_arg_to_janet_command() {
    janet_test!(ed, {
        scripting::eval(r#"(command/define "surface-run-cmd" (fn [arg] (editor/echo arg)))"#);
        scripting::eval(r#"(editor/run-command "surface-run-cmd" "via run-command")"#);
        let msg = ed.status_message.as_ref().expect("run-command must have echoed");
        assert_eq!(msg.text, "via run-command");
    });
}

#[test]
fn janet_command_without_args_still_callable() {
    janet_test!(ed, {
        scripting::eval(r#"(command/define "surface-noarg-cmd" (fn [&] (editor/echo "ran")))"#);
        execute_command_line(&mut ed, "surface-noarg-cmd").unwrap();
        let msg = ed.status_message.as_ref().expect("zero-arg command must run");
        assert_eq!(msg.text, "ran");
        assert_eq!(msg.kind, MessageKind::Info);
    });
}

#[test]
fn janet_command_with_fixed_arity_runs_without_args() {
    // A one-parameter command invoked with no argument (the GUI/colon path)
    // must still run: the dispatcher supplies the argument slot, it does not
    // call with zero arguments.
    janet_test!(ed, {
        scripting::eval(r#"(command/define "surface-fixed-arity" (fn [_] (editor/echo "fixed-ran")))"#);
        execute_command_line(&mut ed, "surface-fixed-arity").unwrap();
        let msg = ed.status_message.as_ref()
            .expect("fixed-arity command must run when dispatched with no argument");
        assert_eq!(msg.text, "fixed-ran");
    });
}

#[test]
fn janet_command_errors_are_surfaced() {
    janet_test!(ed, {
        scripting::eval(r#"(command/define "surface-failing" (fn [_] (error "boom")))"#);
        let result = execute_command_line(&mut ed, "surface-failing");
        assert!(result.is_err(),
            "a signaling Janet command must surface an error, not fail silently");
    });
}
